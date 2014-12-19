//! Compiler behavior.

use std::sync::Arc;

use item::{Item, Dependencies};

/// Behavior of a compiler.
///
/// There's a single method that takes a mutable
/// reference to the `Item` being compiled.
pub trait Compile: Send + Sync {
  fn compile(&self, item: &mut Item, dependencies: Option<Dependencies>);
}

impl<F> Compile for F where F: Fn(&mut Item, Option<Dependencies>) + Send + Sync {
  fn compile(&self, item: &mut Item, deps: Option<Dependencies>) {
    (*self)(item, deps);
  }
}

pub enum Link {
  Compiler(Box<Compile + Send + Sync>),
  Barrier,
}

#[deriving(Clone, Copy)]
pub enum Status {
  Paused,
  Done,
}

pub struct Chain {
  chain: Vec<Link>,
}

impl Chain {
  pub fn new() -> Chain {
    Chain { chain: Vec::new() }
  }

  pub fn only<C>(compiler: C) -> Chain
    where C: Compile {
    Chain { chain: vec![Link::Compiler(box compiler as Box<Compile + Send + Sync>)] }
  }

  pub fn link<C>(mut self, compiler: C) -> Chain
    where C: Compile {
    self.chain.push(Link::Compiler(box compiler as Box<Compile + Send + Sync>));
    self
  }

  pub fn barrier(mut self) -> Chain {
    self.chain.push(Link::Barrier);
    self
  }

  pub fn build(self) -> Arc<Vec<Link>> {
    Arc::new(self.chain)
  }
}

/// Chain of compilers.
///
/// Maintains a list of compilers and executes them
/// in the order they were added.
pub struct Compiler {
  pub chain: Arc<Vec<Link>>,
  pub status: Status,
  position: uint,
}

impl Clone for Compiler {
  fn clone(&self) -> Compiler {
    Compiler {
      chain: self.chain.clone(),
      status: self.status.clone(),
      position: self.position.clone(),
    }
  }
}

impl Compiler {
  pub fn new(chain: Arc<Vec<Link>>) -> Compiler {
    Compiler {
      chain: chain,
      position: 0,
      status: Status::Paused,
    }
  }

  pub fn compile(&mut self, item: &mut Item, deps: Option<Dependencies>) {
    let mut slice = self.chain[self.position..].iter();

    for link in slice {
      self.position += 1;

      match *link {
        Link::Compiler(ref compiler) => compiler.compile(item, deps.clone()),
        Link::Barrier => {
          self.status = Status::Paused;
          return;
        },
      }
    }

    self.status = Status::Done;
  }
}

pub fn stub(item: &mut Item, _deps: Option<Dependencies>) {
  println!("no compiler established for: {}", item);
}

/// Compiler that reads the `Item`'s body.
pub fn read(item: &mut Item, _deps: Option<Dependencies>) {
  item.read();
}

/// Compiler that writes the `Item`'s body.
pub fn write(item: &mut Item, _deps: Option<Dependencies>) {
  item.write();
}

/// Compiler that prints the `Item`'s body.
pub fn print(item: &mut Item, _deps: Option<Dependencies>) {
  use std::io::stdio::println;

  if let &Some(ref body) = &item.body {
    println(body.as_slice());
  } else {
    println("no body");
  }
}

