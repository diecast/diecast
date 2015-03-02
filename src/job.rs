use std::fmt;
use std::sync::Arc;

use compiler::{Compile, is_paused};
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
        write!(f, "#{} [{}] {:?}, dependency_count: {} is_paused: {}",
               self.id,
               self.binding,
               self.item,
               self.dependency_count,
               self.is_paused)
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

    pub fn process(&mut self) {
        self.compiler.compile(&mut self.item);

        // TODO: we're still special-casing Chain here, doesn't matter?
        self.is_paused = is_paused(&self.item);
    }
}


