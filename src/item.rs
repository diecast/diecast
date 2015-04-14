//! Compilation unit for the `Generator`.

use anymap::AnyMap;
use std::io::Write;
use std::fmt::{self, Debug};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::path::{PathBuf, Path};

use binding::{self, Bind};

// TODO:
// pinning down the type like this has the effect of also
// pinning down the evaluation implementation no? this contains Arcs,
// for example, which would nto be necessary in a single threaded evaluator?
// perhaps the alternative is an associated type on a trait
// or perhaps Arcs are fine anyways?
// TODO
// I think this should be its own type
pub type Dependencies = Arc<BTreeMap<String, Arc<Bind>>>;

#[derive(Clone)]
pub enum Route {
    Read(PathBuf),
    Write(PathBuf),
    ReadWrite(PathBuf, PathBuf),
}

impl Route {
    pub fn reading(&self) -> Option<&Path> {
        match *self {
            Route::Write(_) => None,
            Route::Read(ref path) | Route::ReadWrite(ref path, _) => Some(path),
        }
    }

    pub fn writing(&self) -> Option<&Path> {
        match *self {
            Route::Read(_) => None,
            Route::Write(ref path) | Route::ReadWrite(_, ref path) => Some(path),
        }
    }

    // routing:
    //
    // reading routes to readwrite
    // writing routes to new write
    // readwrite routes to new write
    pub fn route_to<R>(self, router: R) -> Route
    where R: Fn(&Path) -> PathBuf {
        match self {
            Route::Read(from) => {
                let target = router(&from);
                Route::ReadWrite(from, target)
            },
            Route::Write(to) => Route::Write(router(&to)),
            Route::ReadWrite(from, _) => {
                let target = router(&from);
                Route::ReadWrite(from, target)
            },
        }
    }
}

impl Debug for Route {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Route::Read(ref path) => try!(write!(f, "Reading {}", path.display())),
            Route::Write(ref path) => try!(write!(f, "Writing {}", path.display())),
            Route::ReadWrite(ref from, ref to) => {
                try!(write!(f, "Reading {}, Writing {}", from.display(), to.display()))
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
/// It also includes an [`AnyMap`](http://www.rust-ci.org/chris-morgan/anymap/doc/anymap/struct.AnyMap.html) which is used to represent miscellaneous data.

#[derive(Clone)]
pub struct Item {
    bind: Arc<binding::Data>,

    route: Route,

    from: Option<PathBuf>,
    to: Option<PathBuf>,

    /// The Item's body which will fill the target file.
    pub body: String,

    /// Any additional data
    pub extensions: AnyMap,
}

// TODO: Item::from and Item::to
impl Item {
    pub fn new(route: Route, bind: Arc<binding::Data>) -> Item {
        use std::fs::PathExt;

        if let Route::Read(ref from) = route {
            assert!(bind.configuration.input.join(&from).is_file())
        }

        let from = route.reading().map(|path| bind.configuration.input.join(&path));
        let to = route.writing().map(|path| bind.configuration.output.join(&path));

        // ensure that the source is a file
        Item {
            route: route,
            from: from,
            to: to,
            body: String::new(),
            extensions: AnyMap::new(),
            bind: bind,
        }
    }

    pub fn from(path: PathBuf, bind: Arc<binding::Data>) -> Item {
        Item::new(Route::Read(path), bind)
    }

    pub fn to(path: PathBuf, bind: Arc<binding::Data>) -> Item {
        Item::new(Route::Write(path), bind)
    }

    fn update_paths(&mut self) {
        self.from = self.route.reading().map(|path| {
            self.bind.configuration.input.join(&path)
        });

        self.to = self.route.writing().map(|path| {
            self.bind.configuration.output.join(&path)
        });
    }

    pub fn route<R>(&mut self, router: R)
    where R: Fn(&Path) -> PathBuf {
        self.route =
            ::std::mem::replace(
                &mut self.route,
                Route::Read(PathBuf::new()))
            .route_to(router);

        self.update_paths();
    }

    pub fn relative_reading(&self) -> Option<&Path> {
        self.route.reading()
    }

    pub fn relative_writing(&self) -> Option<&Path> {
        self.route.writing()
    }

    // TODO:
    // this is a preliminary solution
    // we preferably don't want to keep performing this computation
    // instead it should only done when the route changes and then
    // it should be saved somewhere
    pub fn reading(&self) -> Option<&Path> {
        if let Some(ref from) = self.from {
            Some(from)
        } else {
            None
        }
    }

    pub fn writing(&self) -> Option<&Path> {
        if let Some(ref to) = self.to {
            Some(to)
        } else {
            None
        }
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

