//! item::Handle behavior.

use std::any::Any;
use std::marker::PhantomData;

use handler::Handle;

use typemap;

pub mod item;
pub mod bind;

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

impl<T> Handle<T> for Chain<T> {
    fn handle(&self, t: &mut T) -> ::Result<()> {
        for handler in &self.handlers {
            handler.handle(t)?;
        }

        Ok(())
    }
}

pub fn extend<T>(payload: T::Value) -> Extender<T>
where T: typemap::Key, T::Value: Any + Sync + Send + Clone {
    Extender {
        payload: payload,
    }
}

pub struct Extender<T>
where T: typemap::Key, T::Value: Any + Sync + Send + Clone {
    payload: T::Value,
}

pub struct HandleIf<C, T, H>
where C: Fn(&T) -> bool, C: Sync + Send + 'static,
      H: Handle<T> + Sync + Send + 'static {
    condition: C,
    handler: H,
    _type: PhantomData<T>,
}

impl<C, T, H> Handle<T> for HandleIf<C, T, H>
where C: Fn(&T) -> bool, C: Sync + Send + 'static,
      H: Handle<T> + Sync + Send + 'static {
    fn handle(&self, t: &mut T) -> ::Result<()> {
        if (self.condition)(t) {
            (self.handler.handle(t))
        } else {
            Ok(())
        }
    }
}

#[inline]
pub fn handle_if<C, T, H>(condition: C, handler: H) -> HandleIf<C, T, H>
where C: Fn(&T) -> bool, C: Sync + Send + 'static,
      H: Handle<T> + Sync + Send + 'static {
    HandleIf {
        condition: condition,
        handler: handler,
        _type: PhantomData,
    }
}

