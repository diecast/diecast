//! Compilation unit for the `Generator`.

use anymap::AnyMap;

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

/// An `Item`'s file contents.
pub struct Body(pub String);
