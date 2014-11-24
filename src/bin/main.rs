#![feature(phase)]
#![feature(globs)]

#[phase(plugin, link)]
extern crate diecast;

use diecast::Generator;
use diecast::generator::Binding;
use diecast::compile::{CompilerChain, Read, Print};

fn main() {
  let posts =
    // TODO: impl Pattern for Binding?
    Binding::new("posts")
      .compiler(
        CompilerChain::new()
          .link(Read)
          .link(Print));
      // .router(posts_router);

  let post_index =
    Binding::new("post index")
      .compiler(
        CompilerChain::new()
          .link(Read)
          .link(Print))
      .dependencies(vec!["posts"]);
      // TODO: ^ make possible to just do dependencies(posts)?

  let gen =
    Generator::new(Path::new("tests/fixtures/input"), Path::new("output"))
      .matching("posts/*.md", posts)
      .creating(Path::new("index.html"), post_index);

  println!("generating");

  gen.generate();
}
