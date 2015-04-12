use std::sync::Arc;

use item::Item;
use binding::Bind;

pub type Result = ::std::result::Result<(), Box<::std::error::Error>>;

pub trait Handler<T> {
    fn handle(&self, target: &mut T) -> Result;
}

impl<T, H> Handler<T> for Arc<H> where H: Handler<T> {
    fn handle(&self, target: &mut T) -> Result {
        (**self).handle(target)
    }
}

impl<T> Handler<T> for Box<Handler<T>> {
    fn handle(&self, target: &mut T) -> Result {
        (**self).handle(target)
    }
}

impl<T> Handler<T> for Box<Handler<T> + Sync + Send> {
    fn handle(&self, target: &mut T) -> Result {
        (**self).handle(target)
    }
}

impl<T, F> Handler<T> for F where F: Fn(&mut T) -> Result {
    fn handle(&self, target: &mut T) -> Result {
        self(target)
    }
}

pub struct Chain<T> {
    handlers: Vec<Box<Handler<T> + Sync + Send>>,
}

impl<T> Chain<T> {
    pub fn new() -> Chain<T> {
        Chain {
            handlers: vec![],
        }
    }

    pub fn link<H>(mut self, handler: H) -> Chain<T>
    where H: Handler<T> + Sync + Send + 'static {
        self.handlers.push(Box::new(handler));
        self
    }
}

impl Handler<Item> for Chain<Item> {
    fn handle(&self, item: &mut Item) -> Result {
        for handler in &self.handlers {
            try!(handler.handle(item));
        }

        Ok(())
    }
}

impl Handler<Bind> for Chain<Item> {
    fn handle(&self, binding: &mut Bind) -> Result {
        for item in &mut binding.items {
            try!(<Handler<Item>>::handle(self, item));
        }

        Ok(())
    }
}

impl Handler<Bind> for Chain<Bind> {
    fn handle(&self, binding: &mut Bind) -> Result {
        for handler in &self.handlers {
            try!(handler.handle(binding));
        }

        Ok(())
    }
}

