//! Compilation unit for the `Generator`.

use std::hash::{Hash, Writer};
use anymap::AnyMap;

use std::fmt::{mod, Show};

/// Compilable file.
///
/// This represents a file that can be compiled.
/// It consists of the `Path` to the file, as well
/// as an [`AnyMap`](http://www.rust-ci.org/chris-morgan/anymap/doc/anymap/struct.AnyMap.html),
/// which is a map indexed by a unique type.
pub struct Item {
  /// Path to the file
  pub path: Path,

  /// Any additional data
  pub data: AnyMap
}

// TODO: ensure is_file()
impl Item {
  pub fn new(path: Path) -> Item {
    Item {
      path: path,
      data: AnyMap::new()
    }
  }
}

impl Show for Item {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    try!(self.path.display().fmt(f));
    Ok(())
  }
}

impl PartialEq for Item {
  fn eq(&self, other: &Item) -> bool {
    self.path == other.path
  }

  fn ne(&self, other: &Item) -> bool {
    self.path != other.path
  }
}

impl Eq for Item {}

impl<S> Hash<S> for Item
  where S: Writer {
  fn hash(&self, state: &mut S) {
    self.path.hash(state);
  }
}

/// An `Item`'s file contents.
pub struct Body(pub String);
