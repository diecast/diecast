#![feature(phase)]
#![feature(globs)]
#![feature(if_let)]
#![feature(unboxed_closures)]

#[phase(plugin, link)]
extern crate diecast;

use diecast::Generator;
use diecast::generator::Binding;
use diecast::compile::{CompilerChain, Read, Print};
use diecast::item::Item;


struct DummyValue { age: i32 }

fn read_dummy(item: &mut Item) {
  if let Some(&DummyValue { age }) = item.data.get::<DummyValue>() {
    println!("dummy age is: {}", age);
  }
  else {
    println!("no dummy value!");
  }
}

fn main() {
  let posts =
    // TODO: impl Pattern for Binding?
    Binding::new("posts")
      .compiler(
        CompilerChain::new()
          .link(Read)
          .link(|&: item: &mut Item| { item.data.insert(DummyValue { age: 9 }); })
          .link(read_dummy)
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
