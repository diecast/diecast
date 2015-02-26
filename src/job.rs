use std::fmt;

use compiler::{Compiler, Compile, Status};
use item::Item;

pub struct Job {
    pub id: usize,
    pub binding: &'static str,

    pub item: Item,
    pub compiler: Compiler,
    pub dependency_count: usize,

    pub status: Status,
}

impl fmt::Debug for Job {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "#{} [{}] {:?}, dependency_count: {} status: {:?}",
               self.id,
               self.binding,
               self.item,
               self.dependency_count,
               self.status)
    }
}

impl Job {
    pub fn new(
        binding: &'static str,
        item: Item,
        compiler: Compiler,
        id: usize)
    -> Job {
        Job {
            id: id,
            binding: binding,
            item: item,
            compiler: compiler,
            dependency_count: 0,
            status: Status::Continue,
        }
    }

    pub fn process(&mut self) {
        self.status = self.compiler.compile(&mut self.item);
    }
}


