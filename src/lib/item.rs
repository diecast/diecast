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
  relation: Relation,

  /// The Item's body which will fill the target file.
  body: Option<String>,

  /// Any additional data (post metadata)
  ///
  /// * Title
  /// * Date
  /// * Comments
  /// * TOC
  /// * Tags
  data: AnyMap,
}

// TODO: ensure is_file()
impl Item {
  pub fn new(relation: Relation) -> Item {
    Item {
      relation: relation,
      data: AnyMap::new()
    }
  }

  pub fn relation(&self) -> &Relation {
    &self.relation
  }

  pub fn route_to(mut self, to: Path) {
    if let Reading(from) = self.relation {
      self.relation = Mapping(from, to);
    }
  }
}

pub enum Relation {
  Reading(Path),
  Writing(Path),
  Mapping(Path, Path),
}

impl Show for Item {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self.relation {
      Mapping(from, to) => write!(f, "{} → {}", from.display(), to.display()),
      Reading(from)     => write!(f, "reading {}", from.display()),
      Writing(to)       => write!(f, "writing {}", to.display()),
    }

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
