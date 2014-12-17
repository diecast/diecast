//! Compilation unit for the `Generator`.

use anymap::AnyMap;
use std::io::File;
use std::fmt::{mod, Show};
use std::collections::HashMap;
use std::sync::Arc;

use router::Route;

use self::Relation::*;

// TODO:
pub type Dependencies = Arc<HashMap<&'static str, Arc<Vec<Item>>>>;

#[deriving(Clone)]
pub enum Relation {
  Reading(Path),
  Writing(Path),
  Mapping(Path, Path),
}

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
  pub relation: Relation,

  // pub from: Option<Path>,
  // pub to: Option<Path>,

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
  pub fn new(relation: Relation) -> Item {
    use std::io::fs::PathExtensions;

    // ensure that the source is a file
    match relation {
      Reading(ref from) => assert!(from.is_file()),
      Mapping(ref from, _) => assert!(from.is_file()),
      _ => (),
    }

    Item {
      relation: relation,
      body: None,
      data: AnyMap::new()
    }
  }

  pub fn read(&mut self) {
    match self.relation {
      Reading(ref from) | Mapping(ref from, _) => {
        self.body = File::open(from).read_to_string().ok();
      },
      _ => (),
    }
  }

  pub fn write(&mut self) {
    match self.relation {
      Writing(ref to) | Mapping(_, ref to) => {
        if let Some(ref body) = self.body {
          File::create(to)
            .write_str(body.as_slice())
            .unwrap();
        }
      },
      _ => (),
    }
  }

  // pub fn route<R>(&mut self, router: R) where R: Route {
  //   let to = if let Reading(ref from) = self.relation {
  //     router.route(&from)
  //   };

  //   self.relation = Mapping(from, to);
  // }
}

impl Show for Item {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self.relation {
      Mapping(ref from, ref to) => write!(f, "{} → {}", from.display(), to.display()),
      Reading(ref from)         => write!(f, "reading {}", from.display()),
      Writing(ref to)           => write!(f, "writing {}", to.display()),
    }
  }
}

