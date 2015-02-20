use std::path::{PathBuf, AsPath};
use pattern::Pattern;
use command::CommandKind;

/// The configuration of the build
/// an Arc of this is given to each Item
pub struct Configuration {
    // TODO:
    // I think this shouldn't go here
    // it's possible to use a configuration without
    // running any command at all?
    pub command: CommandKind,

    /// The input directory
    pub input: PathBuf,

    /// The output directory
    pub output: PathBuf,

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

    // Socket on which to listen when in preview mode
    // socket_addr: SocketAddr
}

impl Configuration {
    pub fn new<P: ?Sized, Q: ?Sized>(input: &P, output: &Q) -> Configuration
    where P: AsPath, Q: AsPath {
        Configuration {
            // TODO: setting it to error by default seems like a wart
            command: CommandKind::Other("error".to_string()),
            input: input.as_path().to_path_buf(),
            output: output.as_path().to_path_buf(),
            threads: ::std::os::num_cpus(),
            is_verbose: false,
            ignore: None,
            is_preview: false,
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

    pub fn preview(mut self, is_preview: bool) -> Configuration {
        self.is_preview = is_preview;
        self
    }

    pub fn is_preview(&self) -> bool {
        self.is_preview
    }
}


