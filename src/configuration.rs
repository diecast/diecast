use std::path::{Path, PathBuf};
use std::convert::AsRef;
use std::fs::File;
use std::io::Read;

use num_cpus;
use toml;

use command;
use pattern::Pattern;

/// The configuration of the build
/// an Arc of this is given to each Item
pub struct Configuration {
    toml: toml::Value,

    /// The input directory
    pub input: PathBuf,

    /// The output directory
    pub output: PathBuf,

    pub command: command::Kind,

    /// The number of cpu count
    pub threads: usize,

    pub is_verbose: bool,

    // TODO: necessary?
    // The cache directory
    // cache: PathBuf,

    /// a global pattern used to ignore files and paths
    ///
    /// the following are from hakyll
    /// e.g.
    /// config.ignore = not!(regex!("^\.|^#|~$|\.swp$"))
    pub ignore: Option<Box<Pattern + Sync + Send>>,

    /// Whether we're in preview mode
    pub is_preview: bool,

    /// Whether to ignore hidden files and directories at the
    /// top level of the output directory when cleaning it out
    pub ignore_hidden: bool,

    // Socket on which to listen when in preview mode
    // socket_addr: SocketAddr
}

impl Configuration {
    pub fn new<P: ?Sized, Q: ?Sized>(input: &P, output: &Q) -> Configuration
    where P: AsRef<Path>, Q: AsRef<Path> {
        // if there's no file just set an empty toml table
        // otherwise forcibly attempt to read the contents and parsing them
        // if either of those two fails the program should and will panic
        let toml =
            File::open("config.toml")
            .map(|mut file| {
                let mut contents = String::new();
                file.read_to_string(&mut contents).unwrap();
                contents.parse::<toml::Value>().unwrap()
            })
            .unwrap_or(toml::Value::Table(::std::collections::BTreeMap::new()));

        Configuration {
            toml: toml,
            // TODO: setting it to error by default seems like a wart
            input: input.as_ref().to_path_buf(),
            output: output.as_ref().to_path_buf(),
            command: command::Kind::None,
            threads: num_cpus::get(),
            is_verbose: false,
            ignore: None,
            is_preview: false,
            ignore_hidden: false,
        }
    }

    pub fn toml(&self) -> &toml::Value {
        &self.toml
    }

    pub fn thread_count(mut self, count: usize) -> Configuration {
        self.threads = count;
        self
    }

    pub fn ignore<P>(mut self, pattern: P) -> Configuration
    where P: Pattern + Sync + Send + 'static {
        self.ignore = Some(Box::new(pattern));
        self
    }

    pub fn ignore_hidden(mut self, ignore_hidden: bool) -> Configuration {
        self.ignore_hidden = ignore_hidden;
        self
    }

    pub fn preview(mut self, is_preview: bool) -> Configuration {
        self.is_preview = is_preview;
        self
    }
}

