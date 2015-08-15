use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use std::fmt;
use std::slice;
use std::ops::Deref;

use typemap::TypeMap;

use item::Item;
use configuration::Configuration;

/// Bind data.

#[derive(Clone)]
pub struct Data {
    /// The name of the rule that the bind corresponds to.
    pub name: String,

    /// The bind's dependencies.
    pub dependencies: BTreeMap<String, Arc<Bind>>,

    /// The global configuration
    pub configuration: Arc<Configuration>,

    // TODO: not a fan of exposing the Arc
    /// Arbitrary, bind-level data
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

/// The resulting bind of a `Rule`
///
/// `Bind` represents the resulting bind of a particular `Rule`.

#[derive(Clone)]
pub struct Bind {
    items: Vec<Item>,
    data: Arc<Data>,
}

impl Bind {
    pub fn new(data: Data) -> Bind {
        Bind {
            items: Vec::new(),
            data: Arc::new(data),
        }
    }

    pub fn attach(&mut self, mut item: Item) {
        item.attach_to(self.data.clone());
        self.items.push(item);
    }

    /// Access the bind data as an `Arc`
    pub fn data(&self) -> &Data {
        &self.data
    }

    /// Access the entire set of items mutably
    // TODO rename this
    pub fn items_mut(&mut self) -> &mut Vec<Item> {
        &mut self.items
    }

    /// Access the entire set of items
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// Iterate over the items in the bind.
    pub fn iter<'a>(&'a self) -> Iter<'a> {
        Iter {
            iter: self.items.iter()
        }
    }

    /// Iterate over the mutable items in the bind.
    pub fn iter_mut<'a>(&'a mut self) -> IterMut<'a> {
        IterMut {
            iter: self.items.iter_mut()
        }
    }
}

impl Deref for Bind {
    type Target = Data;

    fn deref<'a>(&'a self) -> &'a Data {
        &self.data
    }
}

pub struct Iter<'a> {
    iter: slice::Iter<'a, Item>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a Item;

    fn next(&mut self) -> Option<&'a Item> {
        self.iter.next()
    }
}

pub struct IterMut<'a> {
    iter: slice::IterMut<'a, Item>,
}

impl<'a> Iterator for IterMut<'a> {
    type Item = &'a mut Item;

    fn next(&mut self) -> Option<&'a mut Item> {
        self.iter.next()
    }
}

impl<'a> IntoIterator for &'a Bind {
    type Item = &'a Item;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut Bind {
    type Item = &'a mut Item;
    type IntoIter = IterMut<'a>;

    fn into_iter(self) -> IterMut<'a> {
        self.iter_mut()
    }
}

impl fmt::Display for Bind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.name.fmt(f)
    }
}

// TODO update for Stale(items)
impl fmt::Debug for Bind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "{}: ", self.name));
        self.items.fmt(f)
    }
}
