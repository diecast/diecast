use std::sync::Arc;

use binding::{self, Bind};
use item::Item;

pub trait Source {
    fn source(&self, bind: Arc<binding::Data>) -> Vec<Item>;
}

impl Source for Box<Source + Sync + Send> {
    fn source(&self, bind: Arc<binding::Data>) -> Vec<Item> {
        (**self).source(bind)
    }
}

impl<F> Source for F
where F: Fn(Arc<binding::Data>) -> Vec<Item> {
fn source(&self, bind: Arc<binding::Data>) -> Vec<Item> {
        self(bind)
    }
}

