//! Compiler behavior.

use item::{Item, Body};
use std::io::File;

/// Behavior of a compiler.
///
/// There's a single method that takes a mutable
/// reference to the `Item` being compiled.
pub trait Compile: Send + Sync {
  fn compile(&self, item: &mut Item);
}

/// Convenience, pass-through implementation to
/// enable trait objects.
impl Compile for Box<Compile + Send + Sync> {
  fn compile(&self, item: &mut Item) {
    (**self).compile(item)
  }
}

/// Chain of compilers.
///
/// Maintains a list of compilers and executes them
/// in the order they were added.
pub struct CompilerChain {
  compilers: Vec<Box<Compile + Send + Sync>>
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

/// Compiler that reads the `Item`'s body.
///
/// Reads the `Item` into its data using the `Body` type.
pub struct ReadBody;

impl Compile for ReadBody {
  fn compile(&self, item: &mut Item) {
    let contents = File::open(&item.path).read_to_string().unwrap();

    item.data.insert(Body(contents));
  }
}

/// Compiler that prints the `Item`'s body.
///
/// Prints the `Body` value in the given `Item`, if found.
pub struct PrintBody;

impl Compile for PrintBody {
  fn compile(&self, item: &mut Item) {
    match item.data.find::<Body>() {
      Some(&Body(ref body)) => {
        println!("printing body");
        println!("{}", body);
      },
      None => println!("no body!"),
    }
  }
}

