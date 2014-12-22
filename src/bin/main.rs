#![feature(phase)]
#![feature(globs)]
#![feature(unboxed_closures)]

#[phase(plugin, link)]
extern crate diecast;
extern crate glob;

extern crate regex;

#[phase(plugin, link)]
extern crate regex_macros;

use diecast::{
  Site,
  Rule,
  Compiler,
  Chain,
  Item,
  Dependencies,
};

use diecast::router;
use diecast::compiler;

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
        .link(compiler::read)
        .link(|&: item: &mut Item, _deps: Option<Dependencies>| {
          item.data.insert(DummyValue { age: 9 });
        })
        .link(read_dummy)
        .link(compiler::print)
        .link(router::SetExtension::new("html"))
        .link(|&: item: &mut Item, _deps: Option<Dependencies>| {
          println!("routed {} -> {}",
                   item.from.clone().unwrap().display(),
                   item.to.clone().unwrap().display());
        })
        .build());

  let posts =
    Rule::new("posts")
      .compiler(content_compiler.clone());

  let post_index =
    Rule::new("post index")
      .depends_on(&posts)
      .compiler(
        Compiler::new(
          Chain::new()
            .link(compiler::read)
            .link(|&: item: &mut Item, deps: Option<Dependencies>| {
              println!("processing {}", item);
              println!("dependencies: {}", deps);
            })
            .link(compiler::print)
            .build()));

  let site =
    Site::new(Path::new("tests/fixtures/input"), Path::new("output"))
      .matching(glob::Pattern::new("posts/*.md"), posts)
      .creating(Path::new("index.html"), post_index);

  // site.build();

  println!("generating");

  let re =
    regex!(
      concat!(
        "(?ms)",
        r"\A---\s*\n",
        r"(?P<metadata>.*?\n?)",
        r"^---\s*$",
        r"\n?",
        r"(?P<body>.*)"));

  let yaml =
r"---
something = lol
another = hah
---

this is the content";

  let captures = re.captures(yaml).unwrap();

  println!("captures\n{}", captures.name("metadata").unwrap());
  println!("body\n{}", captures.name("body").unwrap());
}
