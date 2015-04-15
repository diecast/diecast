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

pub fn extend<T>(payload: T) -> Extender<T>
where T: Sync + Send + Clone + 'static {
    Extender::new(payload)
}

pub struct Extender<T>
where T: Sync + Send + Clone + 'static {
    payload: T,
}

impl<T> Extender<T>
where T: Sync + Send + Clone + 'static {
    pub fn new(data: T) -> Extender<T> {
        Extender {
            payload: data,
        }
    }
}

