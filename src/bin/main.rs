#![feature(phase)]

#[phase(plugin, link)]
extern crate diecast;

use diecast::Generator;
use diecast::compile::{CompilerChain, ReadBody, PrintBody};

fn main() {
  let compiler =
    CompilerChain::new()
      .link(ReadBody)
      .link(PrintBody);

  let mut gen =
    Generator::new(
      Path::new("tests/fixtures/input"),
      Path::new("output"))
      .bind("posts/*", compiler);

  println!("generating");

  gen.generate();
}
