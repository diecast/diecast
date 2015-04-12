use std::sync::Arc;

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

