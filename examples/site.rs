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

use std::fs::File;
use std::io::Read;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::path::{PathBuf, Path};
use rustc_serialize::json::{Json, ToJson};

use regex::Regex;
use hoedown::buffer::Buffer;
use handlebars::Handlebars;
use time::PreciseTime;
use glob::Pattern as Glob;

use diecast::{
    Configuration,
    Rule,
    Item,
    Bind,
};

use diecast::command;
use diecast::util::route;
use diecast::util::handle::{Chain, binding, item};

fn main() {
    env_logger::init().unwrap();

    fn load_template(path: &Path, registry: &mut Handlebars) {
        let mut template = String::new();

        File::open(path)
        .unwrap()
        .read_to_string(&mut template)
        .unwrap();

        let path = path.with_extension("");
        let name = path.file_name().unwrap().to_str().unwrap();

        registry.register_template_string(name, template).unwrap();
    }

    let templates =
        Rule::new("templates")
        .handler(
            Chain::new()
            .link(binding::select("templates/*.html".parse::<Glob>().unwrap()))
            .link(binding::each(item::read))
            .link(|bind: &mut Bind| -> diecast::Result {
                let mut registry = Handlebars::new();

                for item in &bind.items {
                    load_template(item.route.reading().unwrap(), &mut registry);
                }

                bind.data().extensions.write().unwrap().insert::<Arc<Handlebars>>(Arc::new(registry));

                Ok(())
            }));

    let posts_handler =
        binding::parallel_each(Chain::new()
        .link(item::read)
        .link(item::parse_metadata));

    let posts_handler_post =
        binding::parallel_each(Chain::new()
        .link(item::render_markdown)
        .link(route::pretty)
        .link(item::render_template("post", |item: &Item| -> Json {
            let mut bt = BTreeMap::new();

            if let Some(meta) = item.extensions.get::<item::Metadata>() {
                if let Some(body) = item.extensions.get::<Buffer>() {
                    bt.insert("body".to_string(), body.as_str().unwrap().to_json());
                }

                if let Some(title) = meta.data.lookup("title") {
                    bt.insert("title".to_string(), title.as_str().unwrap().to_json());
                }

                if let Some(path) = item.source() {
                    bt.insert("url".to_string(), path.to_str().unwrap().to_json());
                }
            }

            Json::Object(bt)
        }))
        .link(item::render_template("layout", |item: &Item| -> Json {
            let mut bt = BTreeMap::new();

            bt.insert("body".to_string(), item.body.to_json());

            Json::Object(bt)
        }))
        .link(item::write));

    let statics =
        Rule::new("statics")
        .handler(
            Chain::new()
            .link(binding::select(or!(
                "images/**/*".parse::<Glob>().unwrap(),
                "static/**/*".parse::<Glob>().unwrap(),
                "js/**/*".parse::<Glob>().unwrap(),
                "favicon.png",
                "CNAME"
            )))
            .link(
                binding::each(
                    Chain::new()
                    .link(route::identity)
                    .link(item::copy))));

    fn compile_scss(bind: &mut Bind) -> diecast::Result {
        use std::fs;

        trace!("compiling scss");

        let _ = fs::create_dir(bind.data().configuration.output.join("css"));

        // TODO: this needs to be more general, perhaps give it the "screen.css" part
        // as a parameter, and the input/output pair
        try! {
            ::std::process::Command::new("scss")
            .arg("-I")
            .arg(bind.data().configuration.input.join("scss").to_str().unwrap())
            .arg(bind.data().configuration.input.join("scss/screen.scss"))
            .arg(bind.data().configuration.output.join("css/screen.css").to_str().unwrap())
            .status()
        };

        Ok(())
    }

    let scss =
        Rule::new("scss")
        .handler(
            Chain::new()
            .link(binding::select("scss/**/*.scss".parse::<Glob>().unwrap()))
            .link(compile_scss));

    let posts =
        Rule::new("posts")
        .depends_on(&templates)
        .handler(
            Chain::new()
            .link(binding::select("posts/*.markdown".parse::<Glob>().unwrap()))
            .link(posts_handler)
            .link(binding::retain(item::publishable))
            .link(binding::tags)
            .link(posts_handler_post)
            .link(binding::next_prev));

    // this feels awkward
    let index =
        Rule::new("post index")
        .depends_on(&posts)
        .depends_on(&templates)
        .handler(
            Chain::new()
            .link(binding::paginate("posts", 5, |page: usize| -> PathBuf {
                if page == 0 {
                    PathBuf::from("index.html")
                } else {
                    PathBuf::from(&format!("{}/index.html", page))
                }
            }))
            .link(
                binding::parallel_each(Chain::new()
                // TODO: render_template needs a param to determine
                // where the templates reside
                .link(item::render_template("index", |item: &Item| -> Json {
                    let mut bt = BTreeMap::new();

                    let mut items = vec![];

                    let page = item.extensions.get::<item::Page>().unwrap();

                    for post in &item.bind().dependencies["posts"].items[page.range.clone()] {
                        let mut itm = BTreeMap::new();

                        if let Some(meta) = post.extensions.get::<item::Metadata>() {
                            if let Some(title) = meta.data.lookup("title") {
                                itm.insert("title".to_string(), title.as_str().unwrap().to_json());
                            }

                            if let Some(path) = post.source() {
                                itm.insert("url".to_string(), path.parent().unwrap().to_str().unwrap().to_json());
                            }
                        }

                        items.push(itm);
                    }

                    bt.insert("items".to_string(), items.to_json());

                    if let Some((_, ref path)) = page.prev {
                        bt.insert("prev".to_string(), path.parent().unwrap().to_str().unwrap().to_json());
                    }

                    if let Some((_, ref path)) = page.next {
                        bt.insert("next".to_string(), path.parent().unwrap().to_str().unwrap().to_json());
                    }

                    Json::Object(bt)
                }))
                .link(item::render_template("layout", |item: &Item| -> Json {
                    let mut bt = BTreeMap::new();

                    bt.insert("body".to_string(), item.body.to_json());

                    Json::Object(bt)
                }))
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
}

