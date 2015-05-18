use std::sync::Arc;
use std::fmt;

use bind::{self, Bind};
use source::Source;
use handle::{self, Handle};

pub mod evaluator;
mod manager;

pub use self::evaluator::Evaluator;
pub use self::manager::Manager;

pub struct Job {
    pub bind_data: bind::Data,
    pub source: Arc<Box<Source + Sync + Send>>,
    pub handler: Arc<Box<Handle<Bind> + Sync + Send>>,
    pub bind: Option<Bind>,
}

impl fmt::Debug for Job {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{}]", self.bind_data.name)
    }
}

impl Job {
    pub fn new(
        bind: bind::Data,
        source: Arc<Box<Source + Sync + Send>>,
        handler: Arc<Box<Handle<Bind> + Sync + Send>>)
    -> Job {
        Job { bind_data: bind, source: source, handler: handler, bind: None }
    }

    // TODO
    pub fn into_bind(self) -> Bind {
        self.bind.unwrap()
    }

    pub fn process(&mut self) -> handle::Result {
        if let Some(ref mut bind) = self.bind {
            self.handler.handle(bind)
        } else {
            let data = Arc::new(self.bind_data.clone());
            let mut bind =
                Bind::new(self.source.source(data.clone()), data.clone());

            // TODO
            // why not just create an empty Bind and give ref
            // of it to source processors?
            // then source processors can push the items themselves?
            // this would break the manager though

            let res = self.handler.handle(&mut bind);

            self.bind = Some(bind);

            res
        }
    }
}

