//! Compiler behavior.

// use std::collections::ringbuf::RingBuf;
// use std::collections::Deque;

use item::Item;

/// Behavior of a compiler.
///
/// There's a single method that takes a mutable
/// reference to the `Item` being compiled.
pub trait Compile: Send + Sync {
  fn compile(&self, item: &mut Item);
}

/// Chain of compilers.
///
/// Maintains a list of compilers and executes them
/// in the order they were added.
pub struct CompilerChain {
  compilers: Vec<Box<Compile + Send + Sync>>,
  // maintain current progress as an iterator
  // pass: Items<'a, Box<Compile + Send + Sync>>,
}

impl CompilerChain {
  pub fn new() -> CompilerChain {
    CompilerChain { compilers: vec![] }
  }

  /// Add another compiler to the chain.
  ///
  /// ```ignore
  /// let chain =
  ///   CompilerChain::new()
  ///     .link(ReadBody)
  ///     .link(PrintBody);
  /// ```
  pub fn link<C>(mut self, compiler: C) -> CompilerChain
    where C: Compile {
    self.compilers.push(box compiler);
    self
  }
}

impl Compile for CompilerChain {
  fn compile(&self, item: &mut Item) {
    for compiler in self.compilers.iter() {
      compiler.compile(item);
    }
  }
}

pub struct Stub;

impl Compile for Stub {
  fn compile(&self, item: &mut Item) {
    println!("no compiler established for: {}", item);
  }
}

/// Compiler that reads the `Item`'s body.
pub struct Read;

impl Compile for Read {
  fn compile(&self, item: &mut Item) {
    item.read();
  }
}

/// Compiler that writes the `Item`'s body.
pub struct Write;

impl Compile for Write {
  fn compile(&self, item: &mut Item) {
    item.write();
  }
}

/// Compiler that prints the `Item`'s body.
pub struct Print;

impl Compile for Print {
  fn compile(&self, item: &mut Item) {
    if let &Some(ref body) = item.body() {
      ::std::io::stdio::println(body.as_slice());
    } else {
      ::std::io::stdio::println("no body");
    }
  }
}

