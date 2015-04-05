use std::sync::Arc;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::collections::{BTreeMap, VecDeque, HashMap};
use std::mem;
use std::fmt;

use threadpool::ThreadPool;

use binding::{self, Bind};
use dependency::Graph;
use rule::Rule;
use compiler;

pub mod evaluator;
mod manager;

pub use self::evaluator::Evaluator;
pub use self::manager::Manager;

pub struct Job {
    pub bind: Bind,
    pub compiler: Arc<Box<binding::Handler + Sync + Send>>,
}

impl fmt::Debug for Job {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{}]", self.bind.data().name)
    }
}

impl Job {
    pub fn new<C>(bind: Bind, compiler: C) -> Job
    where C: binding::Handler + Sync + Send + 'static {
        Job {
            bind: bind,
            compiler: Arc::new(Box::new(compiler)),
        }
    }

    pub fn into_bind(self) -> Bind {
        self.bind
    }

    pub fn process(&mut self) -> compiler::Result {
        // <Compiler as binding::Handler>::handle(&self.compiler, &mut self.bind)
        self.compiler.handle(&mut self.bind)
    }
}

