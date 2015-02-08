#![feature(plugin)]
#![feature(path)]

#[plugin]
extern crate diecast;
#[plugin]
extern crate regex_macros;
extern crate glob;
extern crate regex;
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate hoedown;

use diecast::{
    Site,
    Rule,
    Compiler,
    Chain,
    Item,
    Dependencies,
};

use diecast::router;
use diecast::compiler::{self, TomlMetadata};
use hoedown::buffer::Buffer;

fn main() {
    env_logger::init().unwrap();

    let content_compiler =
        Compiler::new(
            Chain::new()
            .link(compiler::read)
            .link(compiler::parse_toml)
            .barrier()
            .link(|_item: &mut Item, deps: Option<Dependencies>| {
                trace!("after barrier:\n{:?}", deps);
            })
            .link(|item: &mut Item, _deps: Option<Dependencies>| {
                if let Some(&TomlMetadata(ref meta)) = item.data.get::<TomlMetadata>() {
                    trace!("meta:\n{}", meta);
                }

                trace!("body:\n{:?}", item.body);
            })
            .link(compiler::render_markdown)
            .link(|item: &mut Item, _deps: Option<Dependencies>| {
                println!("{:?}", item.data.get::<Buffer>().unwrap().as_str().unwrap());
            })
            .link(router::SetExtension::new("html"))
            .link(|item: &mut Item, _deps: Option<Dependencies>| {
                trace!("routed {} → {}",
                         item.from.clone().unwrap().display(),
                         item.to.clone().unwrap().display());
            })
            .build());

    let dummy =
        Rule::new("dummy")
        .compiler(Compiler::new(Chain::only(compiler::stub).build()));

    let posts =
        Rule::new("posts")
        .compiler(content_compiler.clone())
        .depends_on(&dummy);

    let post_index =
        Rule::new("index")
        .compiler(Compiler::new(Chain::only(compiler::stub).build()))
        .depends_on(&posts);

    let site =
        Site::new(Path::new("tests/fixtures/input"), Path::new("output"))
        .creating(Path::new("dummy.html"), dummy)
        .matching(glob::Pattern::new("posts/*.md").unwrap(), posts)
        .creating(Path::new("blah.html"), post_index);

    site.build();
}
