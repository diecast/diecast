use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use anymap::Map;
use anymap::any::CloneAny;

use item::{Item, Route};
use configuration::Configuration;

// TODO:
// pinning down the type like this has the effect of also
// pinning down the evaluation implementation no? this contains Arcs,
// for example, which would nto be necessary in a single threaded evaluator?
// perhaps the alternative is an associated type on a trait
// or perhaps Arcs are fine anyways?
// TODO
// I think this should be its own type
pub type Dependencies = BTreeMap<String, Arc<Bind>>;

// FIXME
// problem is that an item handler can easily change
// these fields and essentially corrupt the bind data
// for future items
#[derive(Clone)]
pub struct Data {
    pub name: String,
    pub dependencies: Dependencies,
    pub configuration: Arc<Configuration>,
    pub extensions: Arc<RwLock<Map<CloneAny + Sync + Send>>>,
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
                dependencies: BTreeMap::new(),
                configuration: configuration,
                extensions: Arc::new(RwLock::new(Map::new())),
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

    /// Create and return an item associated with this binding
    pub fn spawn(&mut self, route: Route) -> Item {
        Item::new(route, self.data.clone())
    }

    /// Create and push an item associated with this binding
    pub fn push(&mut self, route: Route) {
        let item = self.spawn(route);
        self.items.push(item);
    }
}

