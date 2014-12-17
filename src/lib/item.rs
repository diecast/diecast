//! Compilation unit for the `Generator`.

use anymap::AnyMap;
use std::io::File;
use std::fmt::{mod, Show};
use std::collections::HashMap;
use std::sync::Arc;

use router::Route;

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

#[deriving(Clone)]
pub struct Item {
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
  pub fn new(from: Option<Path>, to: Option<Path>) -> Item {
    use std::io::fs::PathExtensions;

    if let Some(ref from) = from {
      assert!(from.is_file())
    }

    // ensure that the source is a file
    Item {
      from: from,
      to: to,
      body: None,
      data: AnyMap::new()
    }
  }

  pub fn read(&mut self) {
    match self.from {
      Some(ref path) => {
        self.body = File::open(path).read_to_string().ok();
      },
      _ => (),
    }
  }

  pub fn write(&mut self) {
    match self.to {
      Some(ref path) => {
        if let Some(ref body) = self.body {
          File::create(path)
            .write_str(body.as_slice())
            .unwrap();
        }
      },
      _ => (),
    }
  }

  pub fn route<R>(&mut self, router: R) where R: Route {
    if let Some(ref path) = self.from {
      self.to = Some(router.route(path));
    }
  }
}

impl Show for Item {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "no Show for Item yet")
    // write!(f,"{} → {}", from, to)
  }
}

