#![feature(phase)]

#[phase(plugin, link)]
extern crate diecast;

use diecast::Generator;
use diecast::compile::{CompilerChain, ReadBody, PrintBody};

// fn index_compiler(store: &Store, item: &mut Item) {
//   let posts = store.find("posts/*.md");

//   if let Some(found) = posts {
//     let titles = found.map(|p| p.data.find::<Title>());

//     if let Some(titles) = titles {
//       for Title(text) in titles {
//         println!("- {}", text);
//       }
//     }
//   }
// }

fn main() {
  let posts_compiler =
    CompilerChain::new()
      .link(ReadBody)
      .link(PrintBody);

  let mut gen =
    Generator::new(
      Path::new("tests/fixtures/input"),
      Path::new("output"))
     .bind("posts/*.md", posts_compiler);
    // .create(Path::new("index.html"), index_compiler);

  println!("generating");

  gen.generate();
}
