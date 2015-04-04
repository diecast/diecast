use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use anymap::AnyMap;

use item::{Item, Dependencies};
use configuration::Configuration;
use compiler;

pub struct Data {
    pub name: String,
    pub dependencies: Dependencies,
    pub configuration: Arc<Configuration>,
    pub data: AnyMap,
}

pub struct Bind {
    pub items: Vec<Item>,
    pub data: Arc<RwLock<Data>>,
}

impl Bind {
    // FIXME: I don't like that this has to be associated with the configuration
    pub fn new(name: String, configuration: Arc<Configuration>) -> Bind {
        let data =
            Data {
                name: name,
                dependencies: Arc::new(BTreeMap::new()),
                data: AnyMap::new(),
                configuration: configuration,
            };

        Bind {
            items: Vec::new(),
            data: Arc::new(RwLock::new(data)),
        }
    }

    pub fn push(&mut self, item: Item) {
        self.items.push(item);
    }

    // setters
    pub fn set_dependencies(&mut self, deps: Dependencies) {
        self.data.write().unwrap().dependencies = deps;
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

impl<C: ?Sized> Handler for Box<C> where C: Handler {
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

