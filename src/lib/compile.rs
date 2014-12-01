//! Compiler behavior.

use item::Item;

/// Behavior of a compiler.
///
/// There's a single method that takes a mutable
/// reference to the `Item` being compiled.
pub trait Compile: Send + Sync {
  fn compile(&self, item: &mut Item);
}

// TODO: Arc impl?

impl<F> Compile for F where F: Fn(&mut Item) + Send + Sync {
  fn compile(&self, item: &mut Item) {
    (*self)(item);
  }
}

// TODO: this should be covered by the above someday?
impl Compile for fn(&mut Item) {
  fn compile(&self, item: &mut Item) {
    (*self)(item)
  }
}

enum Link {
  Compiler(Box<Compile + Send + Sync>),
  Barrier,
}

/// Chain of compilers.
///
/// Maintains a list of compilers and executes them
/// in the order they were added.
pub struct Compiler {
  compilers: Vec<Link>,
  // maintain current progress as an iterator
  // pass: Items<'a, Box<Compile + Send + Sync>>,
}

impl Compiler {
  pub fn new() -> Compiler {
    Compiler { compilers: vec![] }
  }

  /// Add another compiler to the chain.
  ///
  /// ```ignore
  /// let chain =
  ///   Compiler::new()
  ///     .link(ReadBody)
  ///     .link(PrintBody);
  /// ```
  pub fn link<C>(mut self, compiler: C) -> Compiler
    where C: Compile {
    self.compilers.push(Link::Compiler(box compiler));
    self
  }

  pub fn barrier(mut self) -> Compiler {
    self.compilers.push(Link::Barrier);
    self
  }
}

impl Compile for Compiler {
  fn compile(&self, item: &mut Item) {
    for compiler in self.compilers.iter() {
      match *compiler {
        Link::Compiler(ref compiler) => compiler.compile(item),
        Link::Barrier => (),
      }
    }
  }
}

pub fn stub(item: &mut Item) {
  println!("no compiler established for: {}", item);
}

/// Compiler that reads the `Item`'s body.
pub fn read(item: &mut Item) {
  item.read();
}

/// Compiler that writes the `Item`'s body.
pub fn write(item: &mut Item) {
  item.write();
}

/// Compiler that prints the `Item`'s body.
pub fn print(item: &mut Item) {
  use std::io::stdio::println;

  if let &Some(ref body) = &item.body {
    println(body.as_slice());
  } else {
    println("no body");
  }
}

