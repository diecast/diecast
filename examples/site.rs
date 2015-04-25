#![feature(collections)]

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

use std::collections::BTreeMap;
use std::path::PathBuf;
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
use diecast::util::handle::{Chain, binding, item};

mod hbs;
mod scss;
mod ws;

fn post_template(item: &Item) -> Json {
    let mut bt = BTreeMap::new();

    if let Some(meta) = item.extensions.get::<item::Metadata>() {
        bt.insert(String::from("body"), item.body.to_json());

        if let Some(title) = meta.data.lookup("title") {
            bt.insert(String::from("title"), title.as_str().unwrap().to_json());
        }

        if let Some(path) = item.route.writing() {
            bt.insert(String::from("url"), path.parent().unwrap().to_str().unwrap().to_json());
        }
    }

    Json::Object(bt)
}

fn index_template(item: &Item) -> Json {
    let page = item.extensions.get::<item::Page>().unwrap();
    let mut bt = BTreeMap::new();
    let mut items = vec![];

    for post in &item.bind().dependencies["posts"].items[page.range.clone()] {
        let mut itm = BTreeMap::new();

        if let Some(meta) = post.extensions.get::<item::Metadata>() {
            if let Some(title) = meta.data.lookup("title") {
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

fn layout_template(item: &Item) -> Json {
    let mut bt = BTreeMap::new();

    bt.insert(String::from("body"), item.body.to_json());

    Json::Object(bt)
}

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
        Rule::new("templates")
        .handler(Chain::new()
            .link(binding::select("templates/*.html".parse::<Glob>().unwrap()))
            .link(binding::each(item::read))
            .link(hbs::register_templates));

    let statics =
        Rule::new("statics")
        .handler(binding::static_file(or!(
            "images/**/*".parse::<Glob>().unwrap(),
            "static/**/*".parse::<Glob>().unwrap(),
            "js/**/*".parse::<Glob>().unwrap(),
            "favicon.png",
            "CNAME"
        )));

    let scss =
        Rule::new("scss")
        .handler(Chain::new()
            .link(binding::select("scss/**/*.scss".parse::<Glob>().unwrap()))
            .link(scss::scss("scss/screen.scss", "css/screen.css")));

    // let pages = _;
    // let notes = _;

    fn git(bind: &mut Bind) -> diecast::Result {
        use std::sync::Arc;
        use std::collections::HashMap;
        use git2::{
            Repository,
            Pathspec,
            Commit,
            DiffOptions,
            Error,
            Diff,
            Oid,
        };

        fn match_with_parent(repo: &Repository, commit: &Commit, parent: &Commit,
                             opts: &mut DiffOptions) -> Result<bool, Error> {
            let a = try!(parent.tree());
            let b = try!(commit.tree());
            let diff = try!(Diff::tree_to_tree(repo, Some(&a), Some(&b), Some(opts)));
            Ok(diff.deltas().len() > 0)
        }

        let repo = Repository::open(".").unwrap();

        let mut cache: HashMap<Oid, Arc<Git>> = HashMap::new();
        let mut input: HashMap<PathBuf, (&mut Item, DiffOptions, Pathspec)> = HashMap::new();

        for item in &mut bind.items {
            let path = item.source().unwrap();

            let mut diffopts = DiffOptions::new();
            diffopts.include_ignored(false);
            diffopts.recurse_ignored_dirs(false);
            diffopts.include_untracked(false);
            diffopts.recurse_untracked_dirs(false);
            diffopts.disable_pathspec_match(true);
            diffopts.enable_fast_untracked_dirs(true);

            diffopts.pathspec(path.to_str().unwrap());

            let pathspec = Pathspec::new(Some(path.to_str().unwrap()).into_iter()).unwrap();

            input.insert(path, (item, diffopts, pathspec));
        }

        let mut prune = vec![];

        let mut revwalk = repo.revwalk().unwrap();
        revwalk.push_head().unwrap();

        macro_rules! filter_try {
            ($e:expr) => (match $e { Ok(t) => t, Err(e) => continue })
        }

        for id in revwalk {
            let commit = filter_try!(repo.find_commit(id));
            let parents = commit.parents().len();

            // TODO: no merge commits?
            if parents > 1 { continue }

            for (path, &mut (ref mut item, ref mut diffopts, ref mut pathspec)) in &mut input {
                let mut is_match = false;

                match commit.parents().len() {
                    0 => {
                        let tree = filter_try!(commit.tree());
                        let flags = git2::PATHSPEC_NO_MATCH_ERROR;
                        if pathspec.match_tree(&tree, flags).is_err() { continue }
                        else { is_match = true; }
                    },
                    _ => {
                        let m = commit.parents().all(|parent| {
                            match_with_parent(&repo, &commit, &parent, diffopts)
                                .unwrap_or(false)
                        });

                        if !m { continue }
                        else { is_match = true; }
                    },
                }

                if is_match {
                    let git =
                        cache.entry(commit.id())
                        .or_insert_with(|| {
                            let message = String::from_utf8_lossy(commit.message_bytes()).into_owned();
                            Arc::new(Git { sha: commit.id(), message: message })
                        })
                        .clone();

                    item.extensions.insert::<Arc<Git>>(git);
                    prune.push(path.clone());
                }
            }

            for path in prune.drain() {
                input.remove(&path).unwrap();
            }
        }

        Ok(())
    }

    #[derive(Clone)]
    struct Git {
        sha: git2::Oid,
        message: String,
    }

    let posts =
        Rule::new("posts")
        .depends_on(&templates)
        .handler(Chain::new()
            .link(binding::select("posts/*.markdown".parse::<Glob>().unwrap()))
            .link(binding::parallel_each(Chain::new()
                .link(item::read)
                .link(item::parse_metadata)
                .link(item::date)))
            .link(binding::retain(item::publishable))
            .link(binding::tags)
            .link(binding::parallel_each(Chain::new()
                .link(item::markdown)
                .link(route::pretty)))
            .link(ws::pipe(ws_tx))
            .link(git)
            .link(binding::parallel_each(Chain::new()
                .link(hbs::render_template(&templates, "post", post_template))
                .link(hbs::render_template(&templates, "layout", layout_template))
                .link(item::write)))
            .link(binding::next_prev)
            .link(binding::sort_by(|a, b| {
                let a = a.extensions.get::<chrono::NaiveDate>().unwrap();
                let b = b.extensions.get::<chrono::NaiveDate>().unwrap();
                b.cmp(a)
            })));

    let index =
        Rule::new("post index")
        .depends_on(&posts)
        .depends_on(&templates)
        .handler(Chain::new()
            .link(binding::paginate(&posts, 5, |page: usize| -> PathBuf {
                if page == 0 {
                    PathBuf::from("index.html")
                } else {
                    PathBuf::from(&format!("{}/index.html", page))
                }
            }))
            .link(binding::parallel_each(Chain::new()
                .link(hbs::render_template(&templates, "index", index_template))
                .link(hbs::render_template(&templates, "layout", layout_template))
                .link(item::write))));

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
    command.site().register(posts);
    command.site().register(index);

    let start = PreciseTime::now();

    command.run();

    let end = PreciseTime::now();

    println!("time elapsed: {}", start.to(end));

    // FIXME: main thread doesn't wait for children?
    println!("EXITING");

    pig_handle.kill().unwrap();
}

