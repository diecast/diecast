use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use std::path::PathBuf;
use anymap::AnyMap;

use item::{Item, Dependencies};
use configuration::Configuration;
use compiler;

// FIXME
// problem is that an item handler can easily change
// these fields and essentially corrupt the bind data
// for future items
#[derive(Clone)]
pub struct Data {
    pub name: String,
    pub dependencies: Dependencies,
    pub configuration: Arc<Configuration>,
    pub data: Arc<RwLock<AnyMap>>,
}

pub struct Bind {
    pub items: Vec<Item>,

    data: Arc<Data>,
}

impl Bind {
    // FIXME: I don't like that this has to be associated with the configuration
    pub fn new(name: String, configuration: Arc<Configuration>) -> Bind {
        let data =
            Data {
                name: name,
                dependencies: Arc::new(BTreeMap::new()),
                configuration: configuration,
                data: Arc::new(RwLock::new(AnyMap::new())),
            };

        Bind {
            items: Vec::new(),
            data: Arc::new(data),
        }
    }

    // TODO: audit
    pub fn with_dependencies(bind: Bind, dependencies: Dependencies) -> Bind {
        let mut data = bind.data().clone();
        data.dependencies = dependencies;

        Bind {
            items: bind.items,
            data: Arc::new(data),
        }
    }

    pub fn data(&self) -> &Data {
        &self.data
    }

    // TODO: this isn't thread-safe, does it matter?
    pub fn new_item(&mut self, from: Option<PathBuf>, to: Option<PathBuf>) -> &mut Item {
        self.items.push(Item::new(from, to, self.data.clone()));
        self.items.last_mut().unwrap()
    }
}

pub trait Handler {
    fn handle(&self, bind: &mut Bind) -> compiler::Result;
}

/// Behavior of a handler.
///
/// There's a single method that takes a mutable
/// reference to the `Bind` being handled.
impl<C> Handler for Arc<C> where C: Handler {
    fn handle(&self, bind: &mut Bind) -> compiler::Result {
        (**self).handle(bind)
    }
}

impl Handler for Box<Handler> {
    fn handle(&self, bind: &mut Bind) -> compiler::Result {
        (**self).handle(bind)
    }
}

impl Handler for Box<Handler + Sync + Send> {
    fn handle(&self, bind: &mut Bind) -> compiler::Result {
        (**self).handle(bind)
    }
}

impl<F> Handler for F where F: Fn(&mut Bind) -> compiler::Result {
    fn handle(&self, bind: &mut Bind) -> compiler::Result {
        self(bind)
    }
}

impl<'a, C> Handler for &'a [C] where C: Handler {
    fn handle(&self, bind: &mut Bind) -> compiler::Result {
        for handler in *self {
            try!(handler.handle(bind));
        }

        Ok(())
    }
}

