#![feature(phase)]

#[phase(plugin, link)]
extern crate diecast;

use diecast::Generator;
use diecast::generator::Binding;
use diecast::compile::{CompilerChain, ReadBody, PrintBody};

fn main() {
  let posts_compiler =
    CompilerChain::new()
      .link(ReadBody)
      .link(PrintBody);

  let mut posts =
    Binding::new("posts", Match("posts/*.md")) // TODO: impl Pattern for Binding?
      .compiler(posts_compiler)
      .router(posts_router);

  let mut post_index =
    Binding::new("post index", Create("index.html"))
      .compiler(index_compiler)
      .dependencies(&["posts"]); // TODO: make possible to just do dependencies(posts)?

  let mut gen =
    Generator::new(Path::new("tests/fixtures/input"), Path::new("output"))
      .bind(posts)
      .bind(post_index);

  println!("generating");

  gen.generate();
}
