use std::sync::Arc;
use std::fmt;

use binding::Bind;
use handle::{self, Handle};

pub mod evaluator;
mod manager;

pub use self::evaluator::Evaluator;
pub use self::manager::Manager;

pub struct Job {
    pub bind: Bind,
    pub handler: Arc<Box<Handle<Bind> + Sync + Send>>,
}

impl fmt::Debug for Job {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{}]", self.bind.data().name)
    }
}

impl Job {
    pub fn new<H>(bind: Bind, handler: H) -> Job
    where H: Handle<Bind> + Sync + Send + 'static {
        Job {
            bind: bind,
            handler: Arc::new(Box::new(handler)),
        }
    }

    pub fn into_bind(self) -> Bind {
        self.bind
    }

    pub fn process(&mut self) -> handle::Result {
        // <handler as binding::Handle>::handle(&self.handler, &mut self.bind)
        self.handler.handle(&mut self.bind)
    }
}

