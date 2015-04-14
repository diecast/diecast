use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use anymap::AnyMap;

use item::{Item, Dependencies, Route};
use configuration::Configuration;

// FIXME
// problem is that an item handler can easily change
// these fields and essentially corrupt the bind data
// for future items
#[derive(Clone)]
pub struct Data {
    pub name: String,
    pub dependencies: Dependencies,
    pub configuration: Arc<Configuration>,
    pub extensions: Arc<RwLock<AnyMap>>,
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
                extensions: Arc::new(RwLock::new(AnyMap::new())),
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
    pub fn new_item(&mut self, route: Route) -> &mut Item {
        self.items.push(Item::new(route, self.data.clone()));
        self.items.last_mut().unwrap()
    }
}

