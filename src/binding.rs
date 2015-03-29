use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use anymap::AnyMap;

use item::{self, Item, Dependencies};
use configuration::Configuration;
use compiler;

// TODO
//   - Arc<Configuration>

// TODO
// expose the bind data to the items
//   - name
//   - dependencies
//   - data

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
    pub fn new(name: String, configuration: Arc<Configuration>) -> Bind {
        Bind {
            items: Vec::new(),
            data: Arc::new(RwLock::new(
                    Data {
                        name: name,
                        dependencies: Arc::new(BTreeMap::new()),
                        data: AnyMap::new(),
                        configuration: configuration,
                    })),
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

