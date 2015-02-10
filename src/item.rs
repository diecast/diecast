//! Compilation unit for the `Generator`.

use anymap::AnyMap;
use std::old_io::File;
use std::fmt::{self, Debug};
use std::collections::BTreeMap;
use std::sync::Arc;
use site::Configuration;

// TODO:
pub type Dependencies = Arc<BTreeMap<&'static str, Arc<Vec<Item>>>>;

/// Represents a compilation unit.
///
/// This represents either a file read, a file write, or
/// a mapping from a file read to a file write.
///
/// It includes a body field which represents the read or to-be-written data.
///
/// It also includes an [`AnyMap`](http://www.rust-ci.org/chris-morgan/anymap/doc/anymap/struct.AnyMap.html) which is used to represent miscellaneous data.

// TODO: use a UUID?

#[derive(Clone)]
pub struct Item {
    pub configuration: Arc<Configuration>,

    pub from: Option<Path>,
    pub to: Option<Path>,

    /// The Item's body which will fill the target file.
    pub body: Option<String>,

    /// Any additional data (post metadata)
    ///
    /// * Title
    /// * Date
    /// * Comments
    /// * TOC
    /// * Tags
    pub data: AnyMap,
}

impl Item {
    pub fn new(config: Arc<Configuration>, from: Option<Path>, to: Option<Path>) -> Item {
        use std::old_io::fs::PathExtensions;

        if let Some(ref from) = from {
            assert!(config.input.join(from).is_file())
        }

        // ensure that the source is a file
        Item {
            configuration: config,
            from: from,
            to: to,
            body: None,
            data: AnyMap::new()
        }
    }

    pub fn read(&mut self) {
        if let Some(ref path) = self.from {
            self.body = File::open(&self.configuration.input.join(path)).read_to_string().ok();
        }
    }

    pub fn write(&mut self) {
        if let Some(ref path) = self.to {
            if let Some(ref body) = self.body {
                File::create(path)
                    .write_str(body)
                    .unwrap();
            }
        }
    }
}

impl Debug for Item {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(ref path) = self.from {
            try!(write!(f, "{}", path.display()));
        } else {
            try!(write!(f, "None"));
        }

        try!(write!(f, " → "));

        if let Some(ref path) = self.to {
            try!(write!(f, "{}", path.display()));
        } else {
            try!(write!(f, "None"));
        }

        Ok(())
    }
}

