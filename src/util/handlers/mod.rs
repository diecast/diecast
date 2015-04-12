//! item::Handle behavior.

use handle::Handle;

pub mod item;
pub mod binding;

pub struct Chain<T> {
    handlers: Vec<Box<Handle<T> + Sync + Send>>,
}

impl<T> Chain<T> {
    pub fn new() -> Chain<T> {
        Chain {
            handlers: vec![],
        }
    }

    pub fn link<H>(mut self, handler: H) -> Chain<T>
    where H: Handle<T> + Sync + Send + 'static {
        self.handlers.push(Box::new(handler));
        self
    }
}

pub fn inject_data<T>(payload: T) -> Injector<T>
where T: Sync + Send + Clone + 'static {
    Injector::new(payload)
}

pub struct Injector<T> where T: Sync + Send + Clone + 'static {
    payload: T,
}

impl<T> Injector<T> where T: Sync + Send + Clone + 'static {
    pub fn new(data: T) -> Injector<T> {
        Injector {
            payload: data,
        }
    }
}

