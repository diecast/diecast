//! Compilation unit for the `Generator`.

use anymap::AnyMap;
use std::fs::File;
use std::io::{Read, Write};
use std::fmt::{self, Debug};
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use std::path::PathBuf;

use binding::{self, Bind};
use compiler;

// TODO:
// pinning down the type like this has the effect of also
// pinning down the evaluation implementation no? this contains Arcs,
// for example, which would nto be necessary in a single threaded evaluator?
// perhaps the alternative is an associated type on a trait
// or perhaps Arcs are fine anyways?
// TODO
// I think this should be its own type
pub type Dependencies = Arc<BTreeMap<String, Arc<Bind>>>;

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
    pub bind: Arc<RwLock<binding::Data>>,

    //  TODO
    //  this doesn't feel right
    //  if Create, then from shouldn't exist
    //  if Matching, then both might exist
    //      but it could also be that it's just reading,
    //      not creating
    //  enum Route {
    //      Creating(PathBuf),
    //      Matching(PathBuf, Option<PathBuf>),
    //  }
    //  it seems like routing should only affect Matching's to
    pub from: Option<PathBuf>,
    pub to: Option<PathBuf>,

    // TODO: just make this a straight up string? empty string
    // means no body
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

// TODO: Item::from and Item::to
impl Item {
    pub fn new(
        from: Option<PathBuf>,
        to: Option<PathBuf>,
        bind: Arc<RwLock<binding::Data>>,
    ) -> Item {
        use std::fs::PathExt;

        if let Some(ref from) = from {
            assert!(bind.read().unwrap().configuration.input.join(from).is_file())
        }

        // ensure that the source is a file
        Item {
            from: from,
            to: to,
            body: None,
            data: AnyMap::new(),
            bind: bind,
        }
    }

    pub fn from(path: PathBuf, bind: Arc<RwLock<binding::Data>>) -> Item {
        Item::new(Some(path), None, bind)
    }

    pub fn to(path: PathBuf, bind: Arc<RwLock<binding::Data>>) -> Item {
        Item::new(None, Some(path), bind)
    }

    pub fn bind(&self) -> ::std::sync::RwLockReadGuard<binding::Data> {
        self.bind.read().unwrap()
    }

    pub fn read(&mut self) {
        if let Some(ref path) = self.from {
            let mut buf = String::new();

            File::open(&self.bind().configuration.input.join(path))
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

/// Behavior of a compiler.
///
/// There's a single method that takes a mutable
/// reference to the `Item` being compiled.
pub trait Handler {
    fn handle(&self, item: &mut Item) -> compiler::Result;
}

impl<H> Handler for Arc<H> where H: Handler {
    fn handle(&self, item: &mut Item) -> compiler::Result {
        (**self).handle(item)
    }
}

impl<H: ?Sized> Handler for Box<H> where H: Handler {
    fn handle(&self, item: &mut Item) -> compiler::Result {
        (**self).handle(item)
    }
}

impl<F> Handler for F where F: Fn(&mut Item) -> compiler::Result {
    fn handle(&self, item: &mut Item) -> compiler::Result {
        self(item)
    }
}

// TODO: should this be an impl for [H] or for &[H]?
// FIXME: this can't work because a single H type is chosen
//        which ends up expecting all of the elements to be the same type
impl<'a, H> Handler for &'a [H] where H: Handler {
    fn handle(&self, item: &mut Item) -> compiler::Result {
        for handler in *self {
            try!(handler.handle(item));
        }

        Ok(())
    }
}

