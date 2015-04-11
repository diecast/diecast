use std::sync::Arc;
use std::fmt;

use binding::Bind;
use handler::{self, Handler};

pub mod evaluator;
mod manager;

pub use self::evaluator::Evaluator;
pub use self::manager::Manager;

pub struct Job {
    pub bind: Bind,
    pub compiler: Arc<Box<Handler<Bind> + Sync + Send>>,
}

impl fmt::Debug for Job {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{}]", self.bind.data().name)
    }
}

impl Job {
    pub fn new<C>(bind: Bind, compiler: C) -> Job
    where C: Handler<Bind> + Sync + Send + 'static {
        Job {
            bind: bind,
            compiler: Arc::new(Box::new(compiler)),
        }
    }

    pub fn into_bind(self) -> Bind {
        self.bind
    }

    pub fn process(&mut self) -> handler::Result {
        // <Compiler as binding::Handler>::handle(&self.compiler, &mut self.bind)
        self.compiler.handle(&mut self.bind)
    }
}

