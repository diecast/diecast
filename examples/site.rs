#![feature(plugin)]
#![feature(old_path)]
#![feature(io)]
#![feature(fs)]
#![feature(core)]

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
extern crate "rustc-serialize" as rustc_serialize;

use diecast::{
    // Site,
    Configuration,
    Rule,
    Chain,
    Item,
};

use diecast::router;
use diecast::command;
use diecast::compiler::{self, TomlMetadata};
use hoedown::buffer::Buffer;

use handlebars::Handlebars;
use std::fs::File;
use std::io::Read;
use std::collections::BTreeMap;
use std::sync::Arc;
use rustc_serialize::json::{Json, ToJson};

fn article_handler(item: &Item) -> Json {
    let mut bt: BTreeMap<String, Json> = BTreeMap::new();

    if let Some(&TomlMetadata(ref metadata)) = item.data.get::<TomlMetadata>() {
        if let Some(body) = item.data.get::<Buffer>() {
            bt.insert("body".to_string(), body.as_str().unwrap().to_json());
        }

        if let Some(title) = metadata.lookup("title") {
            bt.insert("title".to_string(), title.as_str().unwrap().to_json());
        }
    }

    Json::Object(bt)
}

// approach: have a wrapper compiler that only performs its inner if the condition is true

fn is_draft(item: &Item) -> bool {
    item.data.get::<TomlMetadata>()
        .map(|meta| {
            let &TomlMetadata(ref meta) = meta;
            meta.lookup("draft")
               .and_then(::toml::Value::as_bool)
               .unwrap_or(false)
        })
        .unwrap_or(false)
}

fn publishable(item: &Item) -> bool {
    !(is_draft(item) && !item.configuration.is_preview)
}

fn collect_titles(item: &mut Item) -> compiler::Result {
    let mut titles = String::new();

    // TODO: just make Dependencies be empty if there are none?
    for post in item.dependencies.get("posts").expect("no 'posts' dependency").iter() {
        if !publishable(post) {
            continue;
        }

        if let Some(&TomlMetadata(ref metadata)) = post.data.get::<TomlMetadata>() {
            if let Some(ref title) = metadata.lookup("title").and_then(|t| t.as_str()) {
                titles.push_str(&format!("> {}\n", title));
            }
        }
    }

    item.body = Some(titles);
    Ok(())
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

    let content_compiler =
        Chain::new()
            .link(compiler::inject_with(template_registry))
            .link(compiler::read)
            .link(compiler::parse_metadata)
            .link(compiler::parse_toml)
            .link(compiler::render_markdown)
            .link(router::set_extension("html"))
            .link(compiler::render_template("article", article_handler))
            .link(
                compiler::only_if(
                    publishable,
                    Chain::new()
                        // .link(compiler::print)
                        .link(compiler::write)));

    let posts =
        Rule::matching(
            "posts",
            glob::Pattern::new("posts/dots.lols").unwrap(),
            content_compiler.clone());

    let index_compiler =
        Chain::new()
            .link(collect_titles)
            .link(compiler::print)
            .link(compiler::write);

    let index =
        Rule::creating(
            "post index",
            "index.html",
            index_compiler)
            .depends_on(&posts);

    let config =
        Configuration::new("tests/fixtures/hakyll", "output")
            .ignore(regex!(r"^\.|^#|~$|\.swp$|4913"));

    let mut command = command::from_args(config);

    command.site().bind(posts);
    command.site().bind(index);

    command.run();
}

