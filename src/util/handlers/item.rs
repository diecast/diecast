use std::sync::Arc;

use toml;

use handle::{self, Handle, Result};
use item::Item;

use super::{Chain, Injector};

impl Handle<Item> for Chain<Item> {
    fn handle(&self, item: &mut Item) -> Result {
        for handler in &self.handlers {
            try!(handler.handle(item));
        }

        Ok(())
    }
}

impl<T> Handle<Item> for Injector<T> where T: Sync + Send + Clone + 'static {
    fn handle(&self, item: &mut Item) -> handle::Result {
        item.data.insert(self.payload.clone());
        Ok(())
    }
}

/// Handle<Item> that reads the `Item`'s body.
pub fn read(item: &mut Item) -> handle::Result {
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

/// Handle<Item> that writes the `Item`'s body.
pub fn write(item: &mut Item) -> handle::Result {
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


/// Handle<Item> that prints the `Item`'s body.
pub fn print(item: &mut Item) -> handle::Result {
    println!("{}", item.body);

    Ok(())
}

#[derive(Clone)]
pub struct Metadata {
    pub data: toml::Value,
}

pub fn parse_metadata(item: &mut Item) -> handle::Result {
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

pub fn render_markdown(item: &mut Item) -> handle::Result {
    use hoedown::Markdown;
    use hoedown::renderer::html;

    let document = Markdown::new(item.body.as_bytes());
    let renderer = html::Html::new(html::Flags::empty(), 0);
    let buffer = document.render_to_buffer(renderer);
    item.data.insert(buffer);

    Ok(())
}

use rustc_serialize::json::Json;
use handlebars::Handlebars;

pub struct RenderTemplate<H>
where H: Fn(&Item) -> Json + Sync + Send + 'static {
    name: &'static str,
    handler: H,
}

impl<H> Handle<Item> for RenderTemplate<H>
where H: Fn(&Item) -> Json + Sync + Send + 'static {
    fn handle(&self, item: &mut Item) -> handle::Result {
        item.body = {
            let data = item.bind().data.read().unwrap();
            let registry = data.get::<Arc<Handlebars>>().unwrap();

            trace!("rendering template for {:?}", item);
            let json = (self.handler)(item);
            registry.render(self.name, &json).unwrap()
        };

        Ok(())
    }
}

#[inline]
pub fn render_template<H>(name: &'static str, handler: H) -> RenderTemplate<H>
where H: Fn(&Item) -> Json + Sync + Send + 'static {
    RenderTemplate {
        name: name,
        handler: handler,
    }
}

