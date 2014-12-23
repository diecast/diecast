#![feature(phase)]
#![feature(globs)]
#![feature(unboxed_closures)]

#[phase(plugin, link)]
extern crate diecast;
#[phase(plugin, link)]
extern crate regex_macros;
extern crate glob;
extern crate term;
extern crate regex;

use diecast::{
  Site,
  Rule,
  Compiler,
  Chain,
  Item,
  Dependencies,
};

use diecast::router;
use diecast::compiler::{mod, TomlMetadata};
use std::fmt::Show;

fn colored<S, M>(
  term: &mut Box<term::Terminal<term::WriterWrapper> + Send>,
  color: term::color::Color,
  status: S,
  message: M,
  ) -> ::std::io::IoResult<()>
where S: Show, M: Show {
  try!(term.reset());
  try!(term.fg(color));
  try!(term.attr(term::attr::Attr::Bold));
  try!(term.write_str(format!("{:>12}", status).as_slice()));
  try!(term.reset());
  try!(term.write_line(format!(" {}", message).as_slice()));
  try!(term.flush());
  Ok(())
}

fn main() {
  let mut t = term::stdout().unwrap();

  let content_compiler =
    Compiler::new(
      Chain::new()
        .link(compiler::read)
        .link(compiler::parse_toml)
        .link(|&: item: &mut Item, _deps: Option<Dependencies>| {
          if let Some(&TomlMetadata(ref meta)) = item.data.get::<TomlMetadata>() {
            println!("meta:\n{}", meta);
          }
          println!("body:\n{}", item.body);
        })
        .link(router::SetExtension::new("html"))
        .link(|&: item: &mut Item, _deps: Option<Dependencies>| {
          println!("routed {} → {}",
                   item.from.clone().unwrap().display(),
                   item.to.clone().unwrap().display());
        })
        .build());

  let posts =
    Rule::new("posts")
      .compiler(content_compiler.clone());

  colored(&mut t, term::color::GREEN, "preparing", "building dependency graph");

  let site =
    Site::new(Path::new("tests/fixtures/input"), Path::new("output"))
      .matching(glob::Pattern::new("posts/*.md"), posts);

  colored(&mut t, term::color::GREEN, "compiling", "building site");

  site.build();
}
