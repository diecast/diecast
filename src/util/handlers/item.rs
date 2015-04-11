use std::any::Any;
use std::sync::Arc;

use toml;

use handler::{self, Handler};
use item::Item;
use binding::Bind;

pub struct ItemChain {
    handlers: Vec<Box<Handler<Item> + Sync + Send>>,
}

impl ItemChain {
    pub fn new() -> ItemChain {
        ItemChain {
            handlers: vec![],
        }
    }

    pub fn link<H>(mut self, compiler: H) -> ItemChain
    where H: Handler<Item> + Sync + Send + 'static {
        self.handlers.push(Box::new(compiler));
        self
    }
}

impl Handler<Item> for ItemChain {
    fn handle(&self, item: &mut Item) -> handler::Result {
        for handler in &self.handlers {
            try!(handler.handle(item));
        }

        Ok(())
    }
}

impl Handler<Bind> for ItemChain {
    fn handle(&self, binding: &mut Bind) -> handler::Result {
        for item in &mut binding.items {
            try!(<Handler<Item>>::handle(self, item));
        }

        Ok(())
    }
}

/// Handler<Item> that reads the `Item`'s body.
pub fn read(item: &mut Item) -> handler::Result {
    use std::fs::File;
    use std::io::Read;

    if let Some(from) = item.route.reading() {
        let mut buf = String::new();

        // TODO: use try!
        File::open(&item.bind().configuration.input.join(from))
            .unwrap()
            .read_to_string(&mut buf)
            .unwrap();

        item.body = buf;
    }

    Ok(())
}

/// Handler<Item> that writes the `Item`'s body.
pub fn write(item: &mut Item) -> handler::Result {
    use std::fs::{self, File};
    use std::io::Write;

    if let Some(to) = item.route.writing() {
        let conf_out = &item.bind().configuration.output;
        let target = conf_out.join(to);

        if !target.starts_with(&conf_out) {
            // TODO
            // should probably return a proper T: Error?
            println!("attempted to write outside of the output directory: {:?}", target);
            ::std::process::exit(1);
        }

        if let Some(parent) = target.parent() {
            trace!("mkdir -p {:?}", parent);

            // TODO: this errors out if the path already exists? dumb
            let _ = fs::create_dir_all(parent);
        }

        let file = conf_out.join(to);

        trace!("writing file {:?}", file);

        File::create(&file)
            .unwrap()
            .write_all(item.body.as_bytes())
            .unwrap();
    }

    Ok(())
}


/// Handler<Item> that prints the `Item`'s body.
pub fn print(item: &mut Item) -> handler::Result {
    println!("{}", item.body);

    Ok(())
}

#[derive(Clone)]
pub struct Metadata {
    pub data: toml::Value,
}

pub fn parse_metadata(item: &mut Item) -> handler::Result {
    // TODO:
    // should probably allow arbitrary amount of
    // newlines after metadata block?
    let re =
        regex!(
            concat!(
                "(?ms)",
                r"\A---\s*\n",
                r"(?P<metadata>.*?\n?)",
                r"^---\s*$",
                r"\n*",
                r"(?P<body>.*)"));

    let body = if let Some(captures) = re.captures(&item.body) {
        if let Some(metadata) = captures.name("metadata") {
            if let Ok(parsed) = metadata.parse() {
                item.data.insert(Metadata { data: parsed });
            }
        }

        captures.name("body").map(|b| b.to_string())
    } else { None };

    if let Some(body) = body {
        item.body = body;
    }

    Ok(())
}

pub fn render_markdown(item: &mut Item) -> handler::Result {
    use hoedown::Markdown;
    use hoedown::renderer::html;

    let document = Markdown::new(item.body.as_bytes());
    let renderer = html::Html::new(html::Flags::empty(), 0);
    let buffer = document.render_to_buffer(renderer);
    item.data.insert(buffer);

    Ok(())
}

pub fn inject_item_data<T>(t: Arc<T>) -> Box<Handler<Item> + Sync + Send>
where T: Any + Sync + Send + 'static {
    Box::new(move |item: &mut Item| -> handler::Result {
        item.data.insert(t.clone());
        Ok(())
    })
}

use rustc_serialize::json::Json;
use handlebars::Handlebars;

pub fn render_template<H>(name: &'static str, handler: H)
    -> Box<Handler<Item> + Sync + Send>
where H: Fn(&Item) -> Json + Sync + Send + 'static {
    Box::new(move |item: &mut Item| -> handler::Result {
        item.body = {
            let data = item.bind().data.read().unwrap();
            let registry = data.get::<Arc<Handlebars>>().unwrap();

            trace!("rendering template for {:?}", item);
            let json = handler(item);
            registry.render(name, &json).unwrap()
        };


        Ok(())
    })
}

