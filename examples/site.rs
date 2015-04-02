#![feature(plugin)]
#![feature(convert)]

#![plugin(regex_macros)]

#[macro_use]
extern crate diecast;
extern crate regex;
extern crate glob;
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate hoedown;
extern crate handlebars;
extern crate toml;
extern crate rustc_serialize;

use diecast::{
    Configuration,
    Rule,
    Bind,
    Item,
};

use diecast::router;
use diecast::command;
use diecast::binding;
use diecast::compiler::{self, Metadata, BindChain, ItemChain, paginate, Page, Pooled};
use hoedown::buffer::Buffer;

use handlebars::Handlebars;
use std::fs::File;
use std::io::Read;
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::path::{Path, PathBuf};
use rustc_serialize::json::{Json, ToJson};

fn article_handler(item: &Item) -> Json {
    let mut bt: BTreeMap<String, Json> = BTreeMap::new();

    if let Some(&Metadata(ref metadata)) = item.data.get::<Metadata>() {
        if let Some(body) = item.data.get::<Buffer>() {
            bt.insert("body".to_string(), body.as_str().unwrap().to_json());
        }

        if let Some(title) = metadata.lookup("title") {
            bt.insert("title".to_string(), title.as_str().unwrap().to_json());
        }
    }

    Json::Object(bt)
}

fn is_draft(item: &Item) -> bool {
    item.data.get::<Metadata>()
        .map(|meta| {
            let &Metadata(ref meta) = meta;
            meta.lookup("draft")
                .and_then(::toml::Value::as_bool)
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

fn publishable(item: &Item) -> bool {
    !(is_draft(item) && !item.bind().configuration.is_preview)
}

fn main() {
    env_logger::init().unwrap();

    let mut layout = String::new();

    File::open("tests/fixtures/input/layouts/article.handlebars")
    .unwrap()
    .read_to_string(&mut layout)
    .unwrap();

    let mut handlebars = Handlebars::new();
    handlebars.register_template_string("article", layout).unwrap();

    let template_registry = Arc::new(handlebars);

    let posts_compiler =
        ItemChain::new()
        // TODO: this should probably be bind-level data
        .link(compiler::inject_with(template_registry))
        .link(compiler::read)
        .link(compiler::parse_metadata);

    let posts_compiler_post =
        ItemChain::new()
        .link(compiler::render_markdown)
        .link(compiler::render_template("article", article_handler))
        .link(router::set_extension("html"))
        .link(compiler::write);

    let posts_pattern = glob::Pattern::new("posts/*.markdown").unwrap();

    let posts =
        Rule::new("posts")
        .compiler(
            BindChain::new()
            .link(compiler::from_pattern(posts_pattern))
            .link(Pooled::new(posts_compiler))
            .link(compiler::retain(publishable))
            .link(Pooled::new(posts_compiler_post)));

    fn router(page: usize) -> PathBuf {
        if page == 0 {
            PathBuf::from("index.html")
        } else {
            PathBuf::from(&format!("{}/index.html", page))
        }
    }

    // this feels awkward
    let index =
        Rule::new("post index")
        .compiler(
            BindChain::new()
            .link(paginate(5, router))
            .link(
                ItemChain::new()
                .link(|item: &mut Item| -> compiler::Result {
                    let page = item.data.remove::<Page>().unwrap();

                    let mut titles = String::new();

                    for post in &item.bind().dependencies["posts"].items[page.range] {
                        if let Some(&Metadata(ref metadata)) = post.data.get::<Metadata>() {
                            if let Some(ref title) = metadata.lookup("title").and_then(|t| t.as_str()) {
                                titles.push_str(&format!("> {}\n", title));
                            }
                        }
                    }

                    item.body = Some(titles);

                    Ok(())
                })
                .link(compiler::print)
                .link(compiler::write)))
        .depends_on(&posts);

    let config =
        Configuration::new("tests/fixtures/hakyll", "output")
        .ignore(regex!(r"^\.|^#|~$|\.swp$|4913"));

    let mut command = command::from_args(config);

    command.site().register(posts);
    command.site().register(index);

    command.run();
}

