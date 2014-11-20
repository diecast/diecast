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
  /// The backing file of the Item.
  ///
  /// It's optional in case there isn't one, such as
  /// when creating a file.
  pub from: Option<Path>,

  /// The Path of the target file.
  ///
  /// It's optional in case one isn't being created.
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

// TODO: ensure is_file()
impl Item {
  pub fn new(from: Option<Path>, to: Option<Path>) -> Item {
    Item {
      from: from,
      to: to,
      data: AnyMap::new()
    }
  }
}

impl Show for Item {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{} → {}", self.from.display(), self.to.display());
    Ok(())
  }
}

impl PartialEq for Item {
  fn eq(&self, other: &Item) -> bool {
    self.from == other.from && self.to == other.to
  }

  fn ne(&self, other: &Item) -> bool {
    self.from != other.from && self.to != other.to
  }
}

impl Eq for Item {}

impl<S> Hash<S> for Item
  where S: Writer {
  fn hash(&self, state: &mut S) {
    self.from.hash(state);
    self.to.hash(state);
  }
}

/// An `Item`'s file contents.
pub struct Body(pub String);
