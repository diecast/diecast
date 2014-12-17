#![feature(phase)]
#![feature(globs)]
#![feature(unboxed_closures)]

#[phase(plugin, link)]
extern crate diecast;
extern crate glob;

use diecast::Generator;
use diecast::generator::Processor;
use diecast::compiler::{Compiler, Chain};
use diecast::compiler::{read, print};
use diecast::item::{Item, Dependencies};

#[deriving(Clone)]
struct DummyValue { age: i32 }

fn read_dummy(item: &mut Item, _deps: Option<Dependencies>) {
  if let Some(&DummyValue { age }) = item.data.get::<DummyValue>() {
    println!("dummy age is: {}", age);
  } else {
    println!("no dummy value!");
  }
}

fn main() {
  let content_compiler =
    Compiler::new(
      Chain::new()
        .link(read)
        .link(|&: item: &mut Item, _deps: Option<Dependencies>| {
          item.data.insert(DummyValue { age: 9 });
        })
        .link(read_dummy)
        .link(print)
        .build());

  let posts =
    Processor::new("posts")
      .compiler(content_compiler.clone());

  let post_index =
    Processor::new("post index")
      .depends_on(&posts)
      .compiler(
        Compiler::new(
          Chain::new()
            .link(read)
            .link(|&: item: &mut Item, deps: Option<Dependencies>| {
              println!("processing {}", item);
              println!("dependencies: {}", deps);
            })
            .link(print)
            .build()));

  let gen =
    Generator::new(Path::new("tests/fixtures/input"), Path::new("output"))
      .matching(glob::Pattern::new("posts/*.md"), posts)
      .creating(Path::new("index.html"), post_index);

  println!("generating");

  gen.build();
}
