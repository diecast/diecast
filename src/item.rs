//! Compilation unit for the `Generator`.

use std::io::Write;
use std::fmt::{self, Debug};
use std::sync::Arc;
use std::path::{PathBuf, Path};

use typemap::{CloneAny, TypeMap};

use bind;

/// The route of an `Item`.
#[derive(Clone)]
pub enum Route {
    /// A file is being read.
    Read(PathBuf),

    /// A file is being created.
    Write(PathBuf),

    /// A file is being read and another is being written to.
    ReadWrite(PathBuf, PathBuf),
}

// TODO
// rename writing/reading methods
// to avoid confusion with Item constructors
// change to from/to?
impl Route {
    /// Whether or not the route is reading from a file.
    pub fn is_reading(&self) -> bool {
        match *self {
            Route::Write(_) => false,
            Route::Read(_) | Route::ReadWrite(..) => true,
        }
    }

    /// Returns the file being read from, if any.
    pub fn reading(&self) -> Option<&Path> {
        match *self {
            Route::Write(_) => None,
            Route::Read(ref path) | Route::ReadWrite(ref path, _) => Some(path),
        }
    }

    /// Whether or not the route is writing to a file.
    pub fn is_writing(&self) -> bool {
        match *self {
            Route::Read(_) => false,
            Route::Write(_) | Route::ReadWrite(..) => true,
        }
    }

    /// Returns the file being written to, if any.
    pub fn writing(&self) -> Option<&Path> {
        match *self {
            Route::Read(_) => None,
            Route::Write(ref path) | Route::ReadWrite(_, ref path) => Some(path),
        }
    }

    /// Apply a router to this route.
    ///
    /// The semantics are as follows:
    ///
    /// * `Read`: results in a `ReadWrite` by applying the router to
    ///   the read path and using the result as the write path
    /// * `Write`: do nothing
    /// * `ReadWrite`: apply router to the read path and overwrite the
    ///   write path with the result
    pub fn route_with<R>(&mut self, router: R)
    where R: Fn(&Path) -> PathBuf {
        use std::mem;

        let current = mem::replace(self, Route::Read(PathBuf::new()));

        *self = match current {
            // a Read turns into a ReadWrite with the result
            // of the router
            Route::Read(from) => {
                let target = router(&from);
                Route::ReadWrite(from, target)
            },

            // a Write isn't be routed
            Route::Write(_) => current,

            // a ReadWrite simply re-routes the source path
            Route::ReadWrite(from, _) => {
                let target = router(&from);
                Route::ReadWrite(from, target)
            },
        };
    }
}

impl Debug for Route {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Route::Read(ref path) => try!(write!(f, "R {}", path.display())),
            Route::Write(ref path) => try!(write!(f, "W {}", path.display())),
            Route::ReadWrite(ref from, ref to) => {
                try!(write!(f, "R {} → W {}", from.display(), to.display()))
            },
        }

        Ok(())
    }
}

/// Represents a file to be processed.

#[derive(Clone)]
pub struct Item {
    /// The data that was read or that is to be written
    pub body: String,

    /// Arbitrary additional data
    pub extensions: TypeMap<CloneAny + Sync + Send>,

    bind: Option<Arc<bind::Data>>,

    route: Route,
}

// TODO
// have Item::read/Item.read that gets delegated
// to by the read/write handlers?
impl Item {
    pub fn new(route: Route) -> Item {
        Item {
            bind: None,
            route: route,

            body: String::new(),
            extensions: TypeMap::custom(),
        }
    }

    pub fn reading<P>(from: P) -> Item
    where P: Into<PathBuf> {
        Item::new(Route::Read(from.into()))
    }

    pub fn writing<P>(to: P) -> Item
    where P: Into<PathBuf> {
        Item::new(Route::Write(to.into()))
    }

    pub fn read_write<R, W>(from: R, to: W) -> Item
    where R: Into<PathBuf>, W: Into<PathBuf> {
        Item::new(Route::ReadWrite(from.into(), to.into()))
    }

    pub fn attach_to(&mut self, bind: Arc<bind::Data>) {
        self.bind = Some(bind);
    }

    /// Access the item's route.
    pub fn route(&self) -> &Route {
        &self.route
    }

    /// Route the item with the given router.
    pub fn route_with<R>(&mut self, router: R)
    where R: Fn(&Path) -> PathBuf {
        self.route.route_with(router)
    }

    /// The path to the underlying file being read.
    pub fn source(&self) -> Option<PathBuf> {
        self.route.reading().map(|from| {
            self.bind.as_ref().map_or_else(
                || from.to_path_buf(),
                |b| b.configuration.input.join(from))
        })
    }

    /// The path to the underlying file being written to.
    pub fn target(&self) -> Option<PathBuf> {
        self.route.writing().map(|to| {
            self.bind.as_ref().map_or_else(
                || to.to_path_buf(),
                |b| b.configuration.output.join(to))
        })
    }

    /// Access the bind's data
    ///
    /// # Panics
    ///
    /// Panics if the `Item` isn't attached to any `Bind`
    pub fn bind(&self) -> &bind::Data {
        self.bind.as_ref().unwrap()
    }
}

impl fmt::Display for Item {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(self, f)
    }
}

impl Debug for Item {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.route.fmt(f)
    }
}

