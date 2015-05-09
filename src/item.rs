//! Compilation unit for the `Generator`.

use std::io::Write;
use std::fmt::{self, Debug};
use std::sync::Arc;
use std::path::{PathBuf, Path};

use typemap::{TypeMap, Key};

use binding;

#[derive(Clone)]
pub enum Route {
    Read(PathBuf),
    Write(PathBuf),
    ReadWrite(PathBuf, PathBuf),
}

impl Route {
    pub fn is_reading(&self) -> bool {
        match *self {
            Route::Write(_) => false,
            Route::Read(_) | Route::ReadWrite(..) => true,
        }
    }

    pub fn reading(&self) -> Option<&Path> {
        match *self {
            Route::Write(_) => None,
            Route::Read(ref path) | Route::ReadWrite(ref path, _) => Some(path),
        }
    }

    pub fn is_writing(&self) -> bool {
        match *self {
            Route::Read(_) => false,
            Route::Write(_) | Route::ReadWrite(..) => true,
        }
    }

    pub fn writing(&self) -> Option<&Path> {
        match *self {
            Route::Read(_) => None,
            Route::Write(ref path) | Route::ReadWrite(_, ref path) => Some(path),
        }
    }

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

/// Represents a compilation unit.
///
/// This represents either a file read, a file write, or
/// a mapping from a file read to a file write.
///
/// It includes a body field which represents the read or to-be-written data.
///
/// It also includes a `TypeMap` which is used to represent miscellaneous data.

#[derive(Clone)]
pub struct Item {
    bind: Arc<binding::Data>,

    pub route: Route,

    /// The Item's body which will fill the target file.
    pub body: String,

    /// Any additional data
    pub extensions: TypeMap<::typemap::CloneAny + Sync + Send>,
}

impl Item {
    pub fn new(route: Route, bind: Arc<binding::Data>) -> Item {
        use std::fs::PathExt;

        if let Some(path) = route.reading() {
            assert!(bind.configuration.input.join(path).is_file())
        }

        // ensure that the source is a file
        Item {
            route: route,
            body: String::new(),
            extensions: TypeMap::custom(),
            bind: bind,
        }
    }

    pub fn source(&self) -> Option<PathBuf> {
        self.route.reading().map(|from| {
            self.bind.configuration.input.join(from)
        })
    }

    pub fn target(&self) -> Option<PathBuf> {
        self.route.writing().map(|to| {
            self.bind.configuration.output.join(to)
        })
    }

    pub fn bind(&self) -> &binding::Data {
        &self.bind
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

