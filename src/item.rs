//! Compilation unit for the `Generator`.

use anymap::AnyMap;
use std::fs::File;
use std::io::{Read, Write};
use std::fmt::{self, Debug};
use std::collections::HashMap;
use std::sync::Arc;
use configuration::Configuration;
use std::path::PathBuf;

// TODO:
pub type Dependencies = Arc<HashMap<&'static str, Arc<Vec<Item>>>>;

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

    pub from: Option<PathBuf>,
    pub to: Option<PathBuf>,

    // TODO: just make this a straight up string? empty string
    // means no body
    /// The Item's body which will fill the target file.
    pub body: Option<String>,

    pub dependencies: Dependencies,

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
    pub fn new(
        config: Arc<Configuration>,
        from: Option<PathBuf>,
        to: Option<PathBuf>)
    -> Item {
        use std::fs::PathExt;

        if let Some(ref from) = from {
            assert!(config.input.join(from).is_file())
        }

        // ensure that the source is a file
        Item {
            configuration: config,
            from: from,
            to: to,
            body: None,
            dependencies: Arc::new(HashMap::new()),
            data: AnyMap::new()
        }
    }

    pub fn read(&mut self) {
        if let Some(ref path) = self.from {
            let mut buf = String::new();

            File::open(&self.configuration.input.join(path))
                .unwrap()
                .read_to_string(&mut buf)
                .unwrap();

            self.body = Some(buf);
        }
    }

    pub fn write(&mut self) {
        if let Some(ref path) = self.to {
            if let Some(ref body) = self.body {
                File::create(path)
                    .unwrap()
                    .write_all(body.as_bytes())
                    .unwrap();
            }
        }
    }
}

impl fmt::Display for Item {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(self, f)
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

