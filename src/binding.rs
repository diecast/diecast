use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use std::slice;
use std::fmt;

use typemap::TypeMap;

use item::Item;
use configuration::Configuration;

#[derive(Clone)]
pub enum Build { Full, Partial(Vec<Item>), }

// FIXME
// problem is that an item handler can easily change
// these fields and essentially corrupt the bind data
// for future items
#[derive(Clone)]
pub struct Data {
    pub name: String,
    pub dependencies: BTreeMap<String, Arc<Bind>>,
    pub configuration: Arc<Configuration>,
    pub extensions: Arc<RwLock<TypeMap<::typemap::CloneAny + Sync + Send>>>,
}

impl Data {
    pub fn new(name: String, configuration: Arc<Configuration>) -> Data {
        Data {
            name: name,
            dependencies: BTreeMap::new(),
            configuration: configuration,
            extensions: Arc::new(RwLock::new(TypeMap::custom())),
        }
    }
}

#[derive(Clone)]
pub struct Bind {
    items: Vec<Item>,
    data: Arc<Data>,
    build: Build,
}

impl Bind {
    // FIXME: I don't like that this has to be associated with the configuration
    pub fn new(items: Vec<Item>, data: Arc<Data>) -> Bind {
        Bind {
            items: items,
            data: data,
            build: Build::Full,
        }
    }

    pub fn update(&mut self, items: Vec<Item>) {
        self.build = Build::Partial(items);
    }

    pub unsafe fn all_items_mut(&mut self) -> &mut Vec<Item> {
        &mut self.items
    }

    pub unsafe fn items_mut(&mut self) -> &mut Vec<Item> {
        match self.build {
            Build::Full => &mut self.items,
            Build::Partial(ref mut items) => items,
        }
    }

    pub fn data(&self) -> &Data {
        &self.data
    }

    pub fn get_data(&self) -> Arc<Data> {
        self.data.clone()
    }
}

impl ::std::ops::Deref for Bind {
    type Target = [Item];

    fn deref(&self) -> &[Item] {
        match self.build {
            Build::Full => &self.items,
            Build::Partial(ref items) => items,
        }
    }
}

impl ::std::ops::DerefMut for Bind {
    fn deref_mut(&mut self) -> &mut [Item] {
        match self.build {
            Build::Full => &mut self.items,
            Build::Partial(ref mut items) => items,
        }
    }
}

impl<'a> IntoIterator for &'a Bind {
    type Item = &'a Item;
    type IntoIter = slice::Iter<'a, Item>;

    fn into_iter(self) -> slice::Iter<'a, Item> {
        match self.build {
            Build::Full => self.items.iter(),
            Build::Partial(ref items) => items.iter(),
        }
    }
}

impl<'a> IntoIterator for &'a mut Bind {
    type Item = &'a mut Item;
    type IntoIter = slice::IterMut<'a, Item>;

    fn into_iter(self) -> slice::IterMut<'a, Item> {
        match self.build {
            Build::Full => self.items.iter_mut(),
            Build::Partial(ref mut items) => items.iter_mut(),
        }
    }
}

// TODO update for Partial(items)
impl fmt::Debug for Bind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.data().name, match self.build {
            Build::Full => format!("full build of {:?}", self.items),
            Build::Partial(ref items) => format!("partial build of {:?}", items),
        })
    }
}

