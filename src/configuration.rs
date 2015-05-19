use std::path::PathBuf;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;

use num_cpus;
use toml;
use regex::Regex;

use pattern::Pattern;

// TODO: audit

/// The configuration of the build
/// an Arc of this is given to each Item
pub struct Configuration {
    toml: toml::Value,

    /// The input directory
    pub input: PathBuf,

    /// The output directory
    pub output: PathBuf,

    // TODO: necessary?
    // The cache directory
    // cache: PathBuf,

    /// The root command that was invoked
    pub command: String,

    /// The number of cpu count
    pub threads: usize,

    // TODO trigger this programmatically to emit trace!()
    /// Verbosity flag
    pub is_verbose: bool,

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
}

// TODO configuration hierarchy
// CLI -> toml -> code -> defaults
impl Configuration {
    pub fn new() -> Configuration {
        // if there's no file just set an empty toml table
        // otherwise forcibly attempt to read the contents and parsing them
        // if either of those two fails the program should and will panic
        let toml =
            File::open("Diecast.toml")
            .map(|mut file| {
                let mut contents = String::new();
                file.read_to_string(&mut contents).unwrap();
                let parsed: toml::Value = contents.parse().unwrap();

                parsed.as_table().expect("configuration must be a table!");

                parsed
            })
            .unwrap_or(toml::Value::Table(BTreeMap::new()));

        let ignore =
            toml.lookup("diecast.ignore")
            .and_then(toml::Value::as_str)
            .and_then(|s| {
                match Regex::new(s) {
                    Ok(r) => Some(Box::new(r) as Box<Pattern + Send + Sync>),
                    Err(e) => {
                        panic!("could not parse regex: {}", e);
                    },
                }
            });

        let input =
            toml.lookup("diecast.input")
            .and_then(toml::Value::as_str)
            .map(PathBuf::from)
            .unwrap_or(PathBuf::from("input"));

        let output =
            toml.lookup("diecast.output")
            .and_then(toml::Value::as_str)
            .map(PathBuf::from)
            .unwrap_or(PathBuf::from("output"));

        Configuration {
            toml: toml,
            // TODO: setting it to error by default seems like a wart
            input: input,
            output: output,
            command: String::new(),
            threads: num_cpus::get(),
            is_verbose: false,
            ignore: ignore,
            is_preview: false,
            ignore_hidden: false,
        }
    }

    pub fn input<P: ?Sized>(mut self, input: P) -> Configuration
    where P: Into<PathBuf> {
        self.input = input.into();
        self
    }

    pub fn output<P: ?Sized>(mut self, output: P) -> Configuration
    where P: Into<PathBuf> {
        self.output = output.into();
        self
    }

    pub fn toml(&self) -> &toml::Value {
        &self.toml
    }

    pub fn toml_mut(&mut self) -> &mut toml::Table {
        if let toml::Value::Table(ref mut map) = self.toml {
            map
        } else {
            panic!("configuration must be a table!")
        }
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

