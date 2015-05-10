#![feature(collections_drain)]

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
extern crate time;
extern crate chrono;
extern crate websocket;
extern crate zmq;
extern crate git2;
extern crate typemap;
extern crate rss;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use rustc_serialize::json::{Json, ToJson};
use std::process::{Command, Child};

use regex::Regex;
use time::PreciseTime;
use glob::Pattern as Glob;

use diecast::{
    Configuration,
    Rule,
    Item,
    Bind,
    Handle,
};

use diecast::command;
use diecast::util::route;
use diecast::util::source;
use diecast::util::handle::{Chain, binding, item};

mod hbs;
mod scss;
mod ws;
mod feed;
mod git;

#[derive(Clone)]
pub struct Tag {
    pub tag: String,
    pub items: Arc<Vec<Arc<Item>>>,
}

impl typemap::Key for Tag {
    type Value = Tag;
}

fn post_template(item: &Item) -> Json {
    let mut bt = BTreeMap::new();

    // TODO: don't predicate these things on metadata existing
    if let Some(meta) = item.extensions.get::<item::Metadata>() {
        bt.insert(String::from("body"), item.body.to_json());

        if let Some(title) = meta.lookup("title") {
            bt.insert(String::from("title"), title.as_str().unwrap().to_json());
        }

        if let Some(path) = item.route.writing() {
            bt.insert(String::from("url"), path.parent().unwrap().to_str().unwrap().to_json());
        }

        if let Some(date) = item.extensions.get::<item::Date>() {
            bt.insert(String::from("date"), date.format("%B %e, %Y").to_string().to_json());
        }

        if let Some(git) = item.extensions.get::<git::Git>() {
            let sha = git.sha.to_string().chars().take(7).collect::<String>();
            let path = item.source().unwrap();

            // TODO: change the url and branch when ready
            let res =
                format!(
"<a href=\"https://github.com/blaenk/diecast/commits/master/{}\">History</a>\
<span class=\"hash\">, \
<a href=\"https://github.com/blaenk/diecast/commit/{}\" title=\"{}\">{}</a>\
</span>",
                path.to_str().unwrap(), sha, git.message, sha);

            bt.insert(String::from("git"), res.to_json());
        }

        if let Some(tags) = meta.lookup("tags").and_then(toml::Value::as_slice) {
            let tags = tags.iter().map(|t| {
                let tag = t.as_str().unwrap();
                let url = tag.chars()
                    .filter_map(|c| {
                        if c.is_alphanumeric() || c.is_whitespace() {
                            let c = c.to_lowercase().next().unwrap();
                            if c.is_whitespace() { Some('-') }
                            else { Some(c) }
                        } else {
                            None
                        }
                    })
                    .collect::<String>();
                // TODO: sanitize the tag url
                format!("<a href=\"/tags/{}\">{}</a>", url, tag)
            })
            .collect::<Vec<String>>();
            bt.insert(String::from("tags"), tags.connect(", ").to_json());
        }
    }

    Json::Object(bt)
}

fn posts_index_template(item: &Item) -> Json {
    let page = item.extensions.get::<item::Page>().unwrap();
    let mut bt = BTreeMap::new();
    let mut items = vec![];

    for post in &item.bind().dependencies["posts"][page.range.clone()] {
        let mut itm = BTreeMap::new();

        if let Some(meta) = post.extensions.get::<item::Metadata>() {
            if let Some(title) = meta.lookup("title") {
                itm.insert(String::from("title"), title.as_str().unwrap().to_json());
            }

            if let Some(path) = post.route.writing() {
                itm.insert(String::from("url"), path.parent().unwrap().to_str().unwrap().to_json());
            }
        }

        items.push(itm);
    }

    bt.insert(String::from("items"), items.to_json());

    if let Some((_, ref path)) = page.prev {
        bt.insert(String::from("prev"), path.parent().unwrap().to_str().unwrap().to_json());
    }

    if let Some((_, ref path)) = page.next {
        bt.insert(String::from("next"), path.parent().unwrap().to_str().unwrap().to_json());
    }

    Json::Object(bt)
}

fn tags_index_template(item: &Item) -> Json {
    // TODO: how to paginate as well??
    let page = item.extensions.get::<item::Page>().unwrap();
    let mut bt = BTreeMap::new();
    let mut items = vec![];
    let mut tg = String::new();

    if let Some(tag) = item.extensions.get::<Tag>() {
        for post in &tag.items[page.range.clone()] {
            let mut itm = BTreeMap::new();

            tg = tag.tag.clone();

            if let Some(meta) = post.extensions.get::<item::Metadata>() {
                if let Some(title) = meta.lookup("title") {
                    itm.insert(String::from("title"), title.as_str().unwrap().to_json());
                }
            }

            if let Some(path) = post.route.writing() {
                itm.insert(String::from("url"), path.parent().unwrap().to_str().unwrap().to_json());
            }

            items.push(itm);
        }
    }

    bt.insert(String::from("tag"), tg.to_json());
    bt.insert(String::from("items"), items.to_json());

    if let Some((_, ref path)) = page.prev {
        bt.insert(String::from("prev"), path.parent().unwrap().to_str().unwrap().to_json());
    }

    if let Some((_, ref path)) = page.next {
        bt.insert(String::from("next"), path.parent().unwrap().to_str().unwrap().to_json());
    }

    Json::Object(bt)
}

fn notes_index_template(item: &Item) -> Json {
    let page = item.extensions.get::<item::Page>().unwrap();
    let mut bt = BTreeMap::new();
    let mut items = vec![];

    for post in &item.bind().dependencies["notes"][page.range.clone()] {
        let mut itm = BTreeMap::new();

        if let Some(meta) = post.extensions.get::<item::Metadata>() {
            if let Some(title) = meta.lookup("title") {
                itm.insert(String::from("title"), title.as_str().unwrap().to_json());
            }

            if let Some(path) = post.route.writing() {
                itm.insert(String::from("url"), path.parent().unwrap().to_str().unwrap().to_json());
            }

            if let Some(date) = item.extensions.get::<item::Date>() {
                bt.insert(String::from("date"), date.format("%B %e, %Y").to_string().to_json());
            }

            if let Some(git) = item.extensions.get::<git::Git>() {
                let sha = git.sha.to_string().chars().take(7).collect::<String>();
                let path = item.source().unwrap();

                // TODO: change the url and branch when ready
                let res =
                    format!(
    "<a href=\"https://github.com/blaenk/diecast/commits/master/{}\">History</a>\
    <span class=\"hash\">, \
    <a href=\"https://github.com/blaenk/diecast/commit/{}\" title=\"{}\">{}</a>\
    </span>",
                    path.to_str().unwrap(), sha, git.message, sha);

                bt.insert(String::from("git"), res.to_json());
            }
        }

        items.push(itm);
    }

    bt.insert(String::from("items"), items.to_json());

    if let Some((_, ref path)) = page.prev {
        bt.insert(String::from("prev"), path.parent().unwrap().to_str().unwrap().to_json());
    }

    if let Some((_, ref path)) = page.next {
        bt.insert(String::from("next"), path.parent().unwrap().to_str().unwrap().to_json());
    }

    Json::Object(bt)
}

fn layout_template(item: &Item) -> Json {
    let mut bt = BTreeMap::new();

    bt.insert(String::from("body"), item.body.to_json());

    // this should probably go in post template handler
    // move partial load to post template
    if let Some(path) = item.route.reading() {
        bt.insert(String::from("path"), path.to_str().unwrap().to_json());
    }

    if let Some(path) = item.route.writing() {
        bt.insert(String::from("url"), format!("{}/", path.parent().unwrap().to_str().unwrap()).to_json());
    }

    Json::Object(bt)
}

// TODO: implement some sort of heartbeat so that the pig
// server dies when this process dies
fn pig() -> Child {
    println!("initializing pig server...");

    Command::new("python")
        .arg("scripts/pig.py")
        .spawn()
        .unwrap()
}

fn main() {
    env_logger::init().unwrap();

    let mut pig_handle = pig();

    let ws_tx = ws::init();

    println!("pig server initialized");

    let templates =
        Rule::read("templates")
        .source(source::select("templates/*.html".parse::<Glob>().unwrap()))
        .handler(Chain::new()
            .link(binding::each(item::read))
            .link(hbs::register_templates));

    let statics =
        Rule::read("statics")
        .source(source::select(or!(
            "images/**/*".parse::<Glob>().unwrap(),
            "static/**/*".parse::<Glob>().unwrap(),
            "js/**/*".parse::<Glob>().unwrap(),
            "favicon.png",
            "CNAME"
        )))
        .handler(binding::each(Chain::new()
            .link(route::identity)
            .link(item::copy)));

    let scss =
        Rule::read("scss")
        .source(source::select("scss/**/*.scss".parse::<Glob>().unwrap()))
        .handler(scss::scss("scss/screen.scss", "css/screen.css"));

    let pages =
        Rule::read("pages")
        .depends_on(&templates)
        .source(source::select("pages/*.markdown".parse::<Glob>().unwrap()))
        .handler(Chain::new()
            .link(binding::parallel_each(Chain::new()
                .link(item::read)
                .link(item::parse_metadata)
                .link(item::date)))
            // TODO: replace with some sort of filter/only_if
            // .link(binding::retain(item::publishable))
            .link(binding::parallel_each(Chain::new()
                .link(item::markdown)
                .link(|item: &mut Item| -> diecast::Result {
                    item.route.route_with(|path: &Path| -> PathBuf {
                        let without = path.with_extension("");
                        let mut result = PathBuf::from(without.file_name().unwrap());
                        result.push("index.html");
                        result
                    });

                    Ok(())
                })))
            .link(ws::pipe(ws_tx.clone()))
            .link(git::git)
            .link(binding::parallel_each(Chain::new()
                .link(hbs::render_template(&templates, "page", post_template))
                .link(hbs::render_template(&templates, "layout", layout_template))
                .link(item::write))));

    let notes =
        Rule::read("notes")
        .depends_on(&templates)
        .source(source::select("notes/*.markdown".parse::<Glob>().unwrap()))
        .handler(Chain::new()
            .link(binding::parallel_each(Chain::new()
                .link(item::read)
                .link(item::parse_metadata)
                .link(item::date)))
            // TODO: replace with some sort of filter/only_if
            // .link(binding::retain(item::publishable))
            .link(binding::parallel_each(Chain::new()
                .link(item::markdown)
                .link(route::pretty)))
            .link(ws::pipe(ws_tx.clone()))
            .link(git::git)
            .link(binding::parallel_each(Chain::new()
                .link(hbs::render_template(&templates, "note", post_template))
                .link(hbs::render_template(&templates, "layout", layout_template))
                .link(item::write)))
            .link(binding::sort_by(|a, b| {
                let a = a.extensions.get::<item::Date>().unwrap();
                let b = b.extensions.get::<item::Date>().unwrap();
                b.cmp(a)
            })));

    let notes_index =
        Rule::create("note index")
        .depends_on(&notes)
        .depends_on(&templates)
        .source(source::paginate(&notes, 5, |page: usize| -> PathBuf {
            if page == 0 {
                PathBuf::from("notes/index.html")
            } else {
                PathBuf::from(&format!("notes/{}/index.html", page))
            }
        }))
        .handler(binding::parallel_each(Chain::new()
            .link(hbs::render_template(&templates, "index", notes_index_template))
            .link(hbs::render_template(&templates, "layout", layout_template))
            .link(item::write)));

    let posts =
        Rule::read("posts")
        .depends_on(&templates)
        .source(source::select("posts/*.markdown".parse::<Glob>().unwrap()))
        .handler(Chain::new()
            .link(binding::parallel_each(Chain::new()
                .link(item::read)
                .link(item::parse_metadata)
                .link(item::date)))
            .link(binding::retain(item::publishable))
            .link(binding::parallel_each(Chain::new()
                .link(item::markdown)
                .link(item::save_version("rendered"))
                .link(route::pretty)))
            // TODO: should be called after routing
            .link(binding::tags)
            .link(ws::pipe(ws_tx))
            .link(git::git)
            .link(binding::parallel_each(Chain::new()
                .link(hbs::render_template(&templates, "post", post_template))
                .link(hbs::render_template(&templates, "layout", layout_template))
                .link(item::write)))
            .link(binding::sort_by(|a, b| {
                let a = a.extensions.get::<item::Date>().unwrap();
                let b = b.extensions.get::<item::Date>().unwrap();
                b.cmp(a)
            })));

    let posts_index =
        Rule::create("post index")
        .depends_on(&posts)
        .depends_on(&templates)
        .source(source::paginate(&posts, 5, |page: usize| -> PathBuf {
            if page == 0 {
                PathBuf::from("index.html")
            } else {
                PathBuf::from(&format!("{}/index.html", page))
            }
        }))
        .handler(binding::parallel_each(Chain::new()
            .link(hbs::render_template(&templates, "index", posts_index_template))
            .link(hbs::render_template(&templates, "layout", layout_template))
            .link(item::write)));

    fn tag_index(bind: Arc<::diecast::binding::Data>) -> Vec<Item> {
        let mut items = vec![];

        if let Some(tags) = bind.dependencies["posts"].data().extensions.read().unwrap().get::<binding::Tags>() {
            for (tag, itms) in tags {
                let url = tag.chars()
                    .filter_map(|c| {
                        let is_ws = c.is_whitespace();
                        if c.is_alphanumeric() || is_ws {
                            let c = c.to_lowercase().next().unwrap();
                            if is_ws { Some('-') }
                            else { Some(c) }
                        } else {
                            None
                        }
                    })
                    .collect::<String>();
                let mut pgs = source::pages(itms.len(), 5, &move |page: usize| -> PathBuf {
                    if page == 0 {
                        PathBuf::from(&format!("tags/{}/index.html", url))
                    } else {
                        PathBuf::from(&format!("tags/{}/{}/index.html", url, page))
                    }
                }, bind.clone());

                for item in &mut pgs {
                    item.extensions.insert::<Tag>(Tag { tag: tag.clone(), items: itms.clone() });
                }

                items.extend(pgs);
            }
        }

        items
    }

    // TODO: this should be expressed in such a way that it is possible to paginate
    let tags =
        Rule::create("tag index")
        .depends_on(&templates)
        .depends_on(&posts)
        .source(tag_index)
        .handler(binding::parallel_each(Chain::new()
            .link(hbs::render_template(&templates, "tags", tags_index_template))
            .link(hbs::render_template(&templates, "layout", layout_template))
            .link(item::write)));

    let feed =
        Rule::create("feed")
        .depends_on(&posts)
        .source(source::create(PathBuf::from("rss.xml")))
        .handler(binding::each(Chain::new()
            .link(feed::rss)
            .link(item::write)));

    let config =
        Configuration::new("tests/fixtures/hakyll", "output")
        .ignore(r"^\.|^#|~$|\.swp$|4913".parse::<Regex>().unwrap());

    if let Some(i) = config.toml().lookup("age").and_then(toml::Value::as_integer) {
        println!("age: {}", i);
    } else {
        println!("no config.toml present");
    }

    let mut command = command::from_args(config);

    command.site().register(templates);
    command.site().register(statics);
    command.site().register(scss);
    command.site().register(pages);
    command.site().register(posts);
    command.site().register(posts_index);
    command.site().register(tags);
    command.site().register(notes);
    command.site().register(notes_index);
    command.site().register(feed);

    let start = PreciseTime::now();

    command.run();

    let end = PreciseTime::now();

    println!("time elapsed: {}", start.to(end));

    // FIXME: main thread doesn't wait for children?
    println!("EXITING");

    pig_handle.kill().unwrap();
}

