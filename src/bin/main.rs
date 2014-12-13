#![feature(phase)]
#![feature(globs)]
#![feature(unboxed_closures)]

#[phase(plugin, link)]
extern crate diecast;

use diecast::Generator;
use diecast::generator::Binding;
use diecast::compiler::Chain;
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
  let posts =
    Binding::new("posts")
      .compiler(
        Chain::new()
          .link(read)
          .link(|&: item: &mut Item, _deps: Option<Dependencies>| {
            item.data.insert(DummyValue { age: 9 });
          })
          .barrier()
          .link(|&: _item: &mut Item, deps: Option<Dependencies>| {
            println!("after barrier dependencies: {}", deps)
          })
          .link(read_dummy)
          .link(print));

  let post_index =
    Binding::new("post index")
      .compiler(
        Chain::new()
          .link(read)
          .link(|&: item: &mut Item, deps: Option<Dependencies>| {
            println!("processing {}", item);
            println!("dependencies: {}", deps);
          })
          .link(print))
      .depends_on("posts");
      // TODO: ^ make possible to just do dependencies(posts)?

  let gen =
    Generator::new(Path::new("tests/fixtures/input"), Path::new("output"))
      .matching("posts/*.md", posts)
      .creating(Path::new("index.html"), post_index);

  println!("generating");

  gen.build();
}
