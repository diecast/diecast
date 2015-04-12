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
extern crate time;

use std::fs::File;
use std::io::Read;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::path::PathBuf;
use rustc_serialize::json::{Json, ToJson};

use regex::Regex;
use hoedown::buffer::Buffer;
use handlebars::Handlebars;
use time::PreciseTime;

use diecast::{
    Configuration,
    Rule,
    Item,
};

use diecast::command;
use diecast::util::router;
use diecast::util::handler::{self, Chain};
use diecast::util::handler::binding::{Page, Pooled};
use diecast::util::handler::item::Metadata;

fn main() {
    env_logger::init().unwrap();

    let mut layout = String::new();

    File::open("tests/fixtures/input/layouts/article.handlebars")
    .unwrap()
    .read_to_string(&mut layout)
    .unwrap();

    let mut handlebars = Handlebars::new();
    handlebars.register_template_string("article", layout).unwrap();

    let posts_handler =
        Pooled::new(Chain::new()
        .link(handler::item::read)
        .link(handler::item::parse_metadata));

    let posts_handler_post =
        Pooled::new(Chain::new()
        .link(handler::item::render_markdown)
        .link(handler::item::render_template("article", |item: &Item| -> Json {
            let mut bt: BTreeMap<String, Json> = BTreeMap::new();

            if let Some(meta) = item.data.get::<Metadata>() {
                if let Some(body) = item.data.get::<Buffer>() {
                    bt.insert("body".to_string(), body.as_str().unwrap().to_json());
                }

                if let Some(title) = meta.data.lookup("title") {
                    bt.insert("title".to_string(), title.as_str().unwrap().to_json());
                }
            }

            Json::Object(bt)
        }))
        .link(router::set_extension("html"))
        .link(|item: &mut Item| -> diecast::Result {
            trace!("item data for {:?}:", item);
            trace!("body:\n{}", item.body);
            Ok(())
        })
        .link(handler::item::write));

    let posts_pattern = glob::Pattern::new("posts/*.markdown").unwrap();

    let posts =
        Rule::new("posts")
        .handler(
            Chain::new()
            .link(handler::binding::select(posts_pattern))
            .link(handler::inject_data(Arc::new(handlebars)))
            .link(posts_handler)
            .link(handler::binding::retain(handler::item::publishable))
            .link(handler::binding::tags)
            .link(posts_handler_post)
            .link(handler::binding::next_prev));

    // this feels awkward
    let index =
        Rule::new("post index")
        .handler(
            Chain::new()
            .link(handler::binding::paginate("posts", 5, |page: usize| -> PathBuf {
                if page == 0 {
                    PathBuf::from("index.html")
                } else {
                    PathBuf::from(&format!("{}/index.html", page))
                }
            }))
            .link(
                Pooled::new(Chain::new()
                .link(|item: &mut Item| -> diecast::Result {
                    let page = item.data.remove::<Page>().unwrap();

                    let mut titles = String::new();

                    for post in &item.bind().dependencies["posts"].items[page.range] {
                        if let Some(meta) = post.data.get::<Metadata>() {
                            let meta =
                                meta.data.lookup("title").and_then(|t| t.as_str()) ;

                            if let Some(title) = meta {
                                titles.push_str(&format!("> {}\n", title));
                            }
                        }
                    }

                    item.body = titles;

                    Ok(())
                })
                .link(handler::item::print)
                .link(handler::item::write))))
        .depends_on(&posts);

    let config =
        Configuration::new("tests/fixtures/hakyll", "output")
        .ignore(Regex::new(r"^\.|^#|~$|\.swp$|4913").unwrap());

    if let Some(i) = config.toml().lookup("age").and_then(toml::Value::as_integer) {
        println!("age: {}", i);
    } else {
        println!("no config.toml present");
    }

    let mut command = command::from_args(config);

    command.site().register(posts);
    command.site().register(index);

    let start = PreciseTime::now();

    command.run();

    let end = PreciseTime::now();

    println!("time elapsed: {}", start.to(end));

    // FIXME: main thread doesn't wait for children?
    println!("EXITING");
}

