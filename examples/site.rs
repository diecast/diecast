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
use diecast::compiler::{self, TomlMetadata, Pagination, BindChain, ItemChain};
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
    !(is_draft(item) && !item.bind().configuration.is_preview)
}

fn collect_titles(item: &mut Item) -> compiler::Result {
    let mut titles = String::new();

    for post in &item.bind().dependencies["posts"].items {
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

    let posts_compiler =
        ItemChain::new()
            .link(compiler::inject_with(template_registry))
            .link(compiler::read)

            // these two will be merged
            .link(compiler::parse_metadata)
            .link(compiler::parse_toml)

            .link(compiler::render_markdown)

            .link(compiler::render_template("article", article_handler))
            .link(router::set_extension("html"))

            // TODO: only if publishable
            // .link(compiler::print)
            .link(compiler::write);

    let posts_pattern = glob::Pattern::new("posts/dots.markdown").unwrap();

    let mut posts =
        Rule::matching("posts", posts_pattern)
            .compiler(posts_compiler);

    posts.rules_from_matches(|bind: &Bind| -> Vec<Rule> {
        let mut rules = vec![];

        // sort

        let factor = 10;
        let post_count = bind.items.len();
        let mut page_count = post_count / factor;

        if page_count == 0 {
            page_count = 1;
        }

        println!("page count: {}", page_count);

        let mut current = 0;
        let last = page_count - 1;

        while current < page_count {
            let prev = if current == 0 { None } else { Some(current - 1) };
            let next = if current == last { None } else { Some(current + 1) };

            let start = current * factor;
            let end = ::std::cmp::min(bind.items.len(), (current + 1) * factor);

            let items = &bind.items[start .. end];

            let paths =
                items.iter()
                    // ensures only creatable items are used
                    .filter_map(|i| i.from.clone())
                    .collect::<HashSet<PathBuf>>();

            let chunk =
                Rule::matching(format!("chunk {}", current), paths)
                    .compiler(
                        BindChain::new()
                            .link(|bind: &mut Bind| -> compiler::Result {
                                println!("this chunk has {} items", bind.items.len());
                                println!("items include:\n{:?}", bind.items);
                                Ok(())
                            })
                            .link(move |bind: &mut Bind| -> compiler::Result {
                                // TODO: optimize; don't have here; make customizable
                                let route_path = |num: usize| -> PathBuf {
                                    PathBuf::from(&format!("/posts/{}/index.html", num))
                                };

                                bind.data.write().unwrap().data.insert::<Pagination>(
                                    Pagination {
                                        first_number: 1,
                                        first_path: route_path(1),

                                        last_number: page_count - 1,
                                        last_path: route_path(page_count - 1),

                                        next_number: next,
                                        next_path: next.map(|i| route_path(i)),

                                        curr_number: current,
                                        curr_path: route_path(current),

                                        prev_number: prev,
                                        prev_path: prev.map(|i| route_path(i)),

                                        page_count: page_count,
                                        post_count: post_count,
                                        posts_per_page: factor,
                                    }
                                );
                                Ok(())
                            })
                    )
                    .depends_on("posts");

            let rule_name = format!("page {}", current);
            let file_name = format!("posts-{}.html", current);

            let page =
                Rule::creating(rule_name, &Path::new(&file_name))
                    .compiler(
                        ItemChain::new()
                            .link(|item: &mut Item| -> compiler::Result {
                                println!("item is: {:?}", item);
                                // item.body = Some("test".to_string());
                                Ok(())
                            })
                            .link(compiler::write))
                    .depends_on(&chunk);

            rules.push(chunk);
            rules.push(page);
            current += 1;
        }

        rules
    });

    let config =
        Configuration::new("tests/fixtures/hakyll", "output")
            .ignore(regex!(r"^\.|^#|~$|\.swp$|4913"));

    let mut command = command::from_args(config);

    command.site().register(posts);

    command.run();

    // // this is how the pagination will be called
    // // it is given the binding to be paginated
    // // and a compiler for compiling each individual index page
    // // and a routing function
    // paginate(&posts, page_handler, router);
}

