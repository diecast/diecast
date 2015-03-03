use std::fmt;
use std::sync::Arc;
use std::sync::mpsc::Sender;

use compiler::{self, Compile, is_paused};
use item::Item;

pub struct Job {
    pub id: usize,
    pub binding: &'static str,

    pub item: Item,
    pub compiler: Arc<Box<Compile>>,
    pub dependency_count: usize,

    pub is_paused: bool,
}

impl fmt::Debug for Job {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}. [{}]: {:?}",
               self.id,
               self.binding,
               self.item)
    }
}

impl Job {
    pub fn new(
        binding: &'static str,
        item: Item,
        compiler: Arc<Box<Compile>>,
        id: usize)
    -> Job {
        Job {
            id: id,
            binding: binding,
            item: item,
            compiler: compiler,
            dependency_count: 0,
            is_paused: false,
        }
    }

    pub fn process(mut self, tx: Sender<Result<Job, Error>>) {
        // FIXME: this should actually be returned
        match self.compiler.compile(&mut self.item) {
            Ok(()) => {
                // TODO: we're still special-casing Chain here, doesn't matter?
                self.is_paused = is_paused(&self.item);

                tx.send(Ok(self)).unwrap()
            },
            Err(e) => {
                println!("the following job encountered an error:\n  {:?}", self);
                println!("{}", e);
                tx.send(Err(Error)).unwrap();
            }
        }
    }
}

pub struct Error;

