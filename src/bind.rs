use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use std::fmt;
use std::slice;
use std::ops::Deref;

use typemap::TypeMap;

use item::{self, Item, Route};
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
    is_stale: bool,
}

// TODO: this should be private
pub fn set_stale(bind: &mut Bind, is_stale: bool) {
    // if we're setting stale = false
    // and it previously was stale,
    // reset each item
    if !is_stale && bind.is_stale() {
        for item in bind.iter_mut() {
            item::set_stale(item, false);
        }
    }

    bind.is_stale = is_stale;
}

pub fn get_data(bind: &Bind) -> &Data {
    &bind.data
}

impl Bind {
    pub fn new(data: Data) -> Bind {
        Bind {
            items: Vec::new(),
            data: Arc::new(data),
            is_stale: false,
        }
    }

    pub fn spawn(&self, route: Route) -> Item {
        Item::new(route, self.data.clone())
    }

    // TODO audit
    // TODO I would much rather not expose this
    /// Access the bind data as an `Arc`
    pub fn data(&self) -> &Data {
        &self.data
    }

    // TODO this probably shouldn't be here
    /// Whether a bind is out-dated
    pub fn is_stale(&self) -> bool {
        self.is_stale
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
    ///
    /// Note that this possibly only yields the items that have become
    /// outdated. Normally this shouldn't matter. If you do need access
    /// to all of the items, use the `items`/`items_mut` methods.
    pub fn iter<'a>(&'a self) -> Iter<'a> {
        if !self.is_stale {
            Iter {
                iter: IterKind::Full(self.items.iter())
            }
        } else {
            Iter {
                iter: IterKind::Stale(StaleIter {
                    iter: self.items.iter(),
                })
            }
        }
    }

    /// Iterate over the mutable items in the bind.
    ///
    /// Note that this possibly only yields the items that have become
    /// outdated. Normally this shouldn't matter. If you do need access
    /// to all of the items, use the `items`/`items_mut` methods.
    pub fn iter_mut<'a>(&'a mut self) -> IterMut<'a> {
        if !self.is_stale {
            IterMut {
                iter: IterKindMut::Full(self.items.iter_mut())
            }
        } else {
            IterMut {
                iter: IterKindMut::Stale(StaleIterMut {
                    iter: self.items.iter_mut(),
                })
            }
        }
    }
}

impl Deref for Bind {
    type Target = Data;

    fn deref<'a>(&'a self) -> &'a Data {
        &self.data
    }
}

struct StaleIter<'a> {
    iter: slice::Iter<'a, Item>,
}

impl<'a> Iterator for StaleIter<'a> {
    type Item = &'a Item;

    fn next(&mut self) -> Option<&'a Item> {
        while let Some(item) = self.iter.next() {
            if !item.is_stale() {
                continue;
            }

            return Some(item);
        }

        return None;
    }
}

struct StaleIterMut<'a> {
    iter: slice::IterMut<'a, Item>,
}

impl<'a> Iterator for StaleIterMut<'a> {
    type Item = &'a mut Item;

    fn next(&mut self) -> Option<&'a mut Item> {
        while let Some(item) = self.iter.next() {
            if !item.is_stale() {
                continue;
            }

            return Some(item);
        }

        return None;
    }
}

enum IterKind<'a> {
    Full(slice::Iter<'a, Item>),
    Stale(StaleIter<'a>)
}

pub struct Iter<'a> {
    iter: IterKind<'a>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a Item;

    fn next(&mut self) -> Option<&'a Item> {
        match self.iter {
            IterKind::Full(ref mut i) => i.next(),
            IterKind::Stale(ref mut i) => i.next(),
        }
    }
}

enum IterKindMut<'a> {
    Full(slice::IterMut<'a, Item>),
    Stale(StaleIterMut<'a>)
}

pub struct IterMut<'a> {
    iter: IterKindMut<'a>,
}

impl<'a> Iterator for IterMut<'a> {
    type Item = &'a mut Item;

    fn next(&mut self) -> Option<&'a mut Item> {
        match self.iter {
            IterKindMut::Full(ref mut i) => i.next(),
            IterKindMut::Stale(ref mut i) => i.next(),
        }
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
        write!(f, "{}: {:?}", self.name, self.items)
    }
}

