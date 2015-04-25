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
    pub fn new(bind: Bind, handler: Arc<Box<Handle<Bind> + Sync + Send>>) -> Job {
        Job {
            bind: bind,
            handler: handler,
        }
    }

    pub fn into_bind(self) -> Bind {
        self.bind
    }

    pub fn process(&mut self) -> handle::Result {
        self.handler.handle(&mut self.bind)
    }
}

