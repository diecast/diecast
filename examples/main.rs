#![feature(plugin)]
#![feature(path)]

#[plugin]
extern crate diecast;
#[plugin]
extern crate regex_macros;
extern crate glob;
extern crate regex;
extern crate env_logger;

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

fn main() {
    env_logger::init().unwrap();

    let content_compiler =
        Compiler::new(
            Chain::new()
            .link(compiler::read)
            .link(compiler::parse_toml)
            .link(|&: item: &mut Item, _deps: Option<Dependencies>| {
                if let Some(&TomlMetadata(ref meta)) = item.data.get::<TomlMetadata>() {
                    println!("meta:\n{}", meta);
                }

                println!("body:\n{:?}", item.body);
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

    let post_index =
        Rule::new("index")
        .compiler(Compiler::new(Chain::only(compiler::stub).build()))
        .depends_on(&posts);

    let site =
        Site::new(Path::new("tests/fixtures/input"), Path::new("output"))
        .matching(glob::Pattern::new("posts/*.md").unwrap(), posts)
        .creating(Path::new("blah.html"), post_index);

    site.build();
}
