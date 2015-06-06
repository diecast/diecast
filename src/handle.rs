use std::sync::Arc;

pub trait Handle<T> {
    fn handle(&self, target: &mut T) -> ::Result;
}

impl<T, H> Handle<T> for Arc<H>
where H: Handle<T> {
    fn handle(&self, target: &mut T) -> ::Result {
        (**self).handle(target)
    }
}

impl<T> Handle<T> for Box<Handle<T>> {
    fn handle(&self, target: &mut T) -> ::Result {
        (**self).handle(target)
    }
}

impl<T> Handle<T> for Box<Handle<T> + Sync + Send> {
    fn handle(&self, target: &mut T) -> ::Result {
        (**self).handle(target)
    }
}

impl<T, F> Handle<T> for F
where F: Fn(&mut T) -> ::Result {
    fn handle(&self, target: &mut T) -> ::Result {
        self(target)
    }
}

