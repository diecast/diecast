use std::sync::Arc;

use bind;
use item::Item;

pub trait Source {
    fn source(&self, bind: Arc<bind::Data>) -> Vec<Item>;
}

impl Source for Box<Source + Sync + Send> {
    fn source(&self, bind: Arc<bind::Data>) -> Vec<Item> {
        (**self).source(bind)
    }
}

impl<F> Source for F
where F: Fn(Arc<bind::Data>) -> Vec<Item> {
fn source(&self, bind: Arc<bind::Data>) -> Vec<Item> {
        self(bind)
    }
}

