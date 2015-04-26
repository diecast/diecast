use std::sync::Arc;
use std::fmt;

use binding::{self, Bind};
use source::Source;
use handle::{self, Handle};

pub mod evaluator;
mod manager;

pub use self::evaluator::Evaluator;
pub use self::manager::Manager;

pub struct Job {
    pub bind: binding::Data,
    pub source: Arc<Box<Source + Sync + Send>>,
    pub handler: Arc<Box<Handle<Bind> + Sync + Send>>,
    binding: Option<Bind>,
}

impl fmt::Debug for Job {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{}]", self.bind.name)
    }
}

impl Job {
    pub fn new(
        bind: binding::Data,
        source: Arc<Box<Source + Sync + Send>>,
        handler: Arc<Box<Handle<Bind> + Sync + Send>>)
    -> Job {
        Job { bind: bind, source: source, handler: handler, binding: None }
    }

    // TODO
    pub fn into_bind(self) -> Bind {
        self.binding.unwrap()
    }

    pub fn process(&mut self) -> handle::Result {
        let data = Arc::new(self.bind.clone());
        let mut binding = Bind::new(self.source.source(data.clone()), data.clone());

        let res = self.handler.handle(&mut binding);

        self.binding = Some(binding);

        res
    }
}

