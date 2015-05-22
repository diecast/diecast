use std::sync::Arc;
use std::path::PathBuf;
use std::fmt;

use bind::{self, Bind};
use handle::{self, Handle};
use rule;

pub mod evaluator;
mod manager;

pub use self::evaluator::Evaluator;
pub use self::manager::Manager;

pub struct Job {
    pub bind_data: bind::Data,
    pub kind: Arc<rule::Kind>,
    pub handler: Arc<Box<Handle<Bind> + Sync + Send>>,
    pub bind: Option<Bind>,
    paths: Arc<Vec<PathBuf>>,
}

impl fmt::Debug for Job {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{}]", self.bind_data.name)
    }
}

impl Job {
    pub fn new(
        bind: bind::Data,
        kind: Arc<rule::Kind>,
        handler: Arc<Box<Handle<Bind> + Sync + Send>>,
        paths: Arc<Vec<PathBuf>>)
    -> Job {
        Job { bind_data: bind, kind: kind, handler: handler, bind: None, paths: paths }
    }

    // TODO
    pub fn into_bind(self) -> Bind {
        self.bind.unwrap()
    }

    // TODO: feels weird to have this here
    fn populate(&self, bind: &mut Bind) {
        use item::{Item, Route};
        use support;

        let data = bind.get_data();

        match *self.kind {
            rule::Kind::Creating => (),
            rule::Kind::Matching(ref pattern) => {
                for path in self.paths.iter() {
                    let relative =
                        support::path_relative_from(path, &bind.data().configuration.input).unwrap()
                        .to_path_buf();

                    // TODO: JOIN STANDARDS
                    // should insert path.clone()
                    if pattern.matches(&relative) {
                        bind.items_mut().push(Item::new(Route::Read(relative), data.clone()));
                    }
                }
            },
        }
    }

    pub fn process(&mut self) -> handle::Result {
        if let Some(ref mut bind) = self.bind {
            self.handler.handle(bind)
        } else {
            let mut bind =
                Bind::new(self.bind_data.clone());

            trace!("populating {:?}", bind);

            self.populate(&mut bind);

            trace!("populated {:?} with {} items", bind, bind.items().len());

            let res = self.handler.handle(&mut bind);

            self.bind = Some(bind);

            res
        }
    }
}

