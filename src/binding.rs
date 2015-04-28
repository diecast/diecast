use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use std::path::PathBuf;
use std::slice;
use std::fmt;

use anymap::Map;
use anymap::any::CloneAny;

use item::{Item, Route};
use configuration::Configuration;

pub enum Build {
    Full,
    Update(PathBuf),
}

// FIXME
// problem is that an item handler can easily change
// these fields and essentially corrupt the bind data
// for future items
#[derive(Clone)]
pub struct Data {
    pub name: String,
    pub dependencies: BTreeMap<String, Arc<Bind>>,
    pub configuration: Arc<Configuration>,
    pub extensions: Arc<RwLock<Map<CloneAny + Sync + Send>>>,
}

impl Data {
    pub fn new(name: String, configuration: Arc<Configuration>) -> Data {
        Data {
            name: name,
            dependencies: BTreeMap::new(),
            configuration: configuration,
            extensions: Arc::new(RwLock::new(Map::new())),
        }
    }
}

#[derive(Clone)]
pub struct Bind {
    items: Vec<Item>,
    data: Arc<Data>,
}

impl Bind {
    // FIXME: I don't like that this has to be associated with the configuration
    pub fn new(items: Vec<Item>, data: Arc<Data>) -> Bind {
        Bind {
            items: items,
            data: data,
        }
    }

    pub unsafe fn items_mut(&mut self) -> &mut Vec<Item> {
        &mut self.items
    }

    pub fn data(&self) -> &Data {
        &self.data
    }

    pub fn get_data(&self) -> Arc<Data> {
        self.data.clone()
    }

    pub fn spawn(&self, route: Route) -> Item {
        Item::new(route, self.data.clone())
    }
}

impl ::std::ops::Deref for Bind {
    type Target = [Item];

    fn deref(&self) -> &[Item] {
        &self.items
    }
}

impl ::std::ops::DerefMut for Bind {
    fn deref_mut(&mut self) -> &mut [Item] {
        &mut self.items
    }
}

impl<'a> IntoIterator for &'a Bind {
    type Item = &'a Item;
    type IntoIter = slice::Iter<'a, Item>;

    fn into_iter(self) -> slice::Iter<'a, Item> {
        self.items.iter()
    }
}

impl<'a> IntoIterator for &'a mut Bind {
    type Item = &'a mut Item;
    type IntoIter = slice::IterMut<'a, Item>;

    fn into_iter(self) -> slice::IterMut<'a, Item> {
        self.items.iter_mut()
    }
}

impl fmt::Debug for Bind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {:?}", self.data().name, self.items)
    }
}

