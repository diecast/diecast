use std::sync::Arc;
use std::path::PathBuf;
use std::ops::Range;

use regex::Regex;
use toml;
use hoedown;
use hoedown::renderer::html;

use handle::{self, Handle, Result};
use item::Item;

use super::{Chain, Extender};

impl Handle<Item> for Chain<Item> {
    fn handle(&self, item: &mut Item) -> Result {
        for handler in &self.handlers {
            try!(handler.handle(item));
        }

        Ok(())
    }
}

impl<T> Handle<Item> for Extender<T>
where T: Sync + Send + Clone + 'static {
    fn handle(&self, item: &mut Item) -> handle::Result {
        item.extensions.insert(self.payload.clone());
        Ok(())
    }
}

pub fn copy(item: &mut Item) -> handle::Result {
    use std::fs;

    if let Some(from) = item.source() {
        if let Some(to) = item.target() {
            // TODO: once path normalization is in, make sure
            // writing to output folder

            if let Some(parent) = to.parent() {
                // TODO: this errors out if the path already exists? dumb
                let _ = fs::create_dir_all(parent);
            }

            try!(fs::copy(from, to));
        }
    }

    Ok(())
}

/// Handle<Item> that reads the `Item`'s body.
pub fn read(item: &mut Item) -> handle::Result {
    use std::fs::File;
    use std::io::Read;

    if let Some(from) = item.source() {
        let mut buf = String::new();

        // TODO: use try!
        File::open(from)
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

    if let Some(to) = item.target() {
        // TODO: once path normalization is in, make sure
        // writing to output folder

        if let Some(parent) = to.parent() {
            // TODO: this errors out if the path already exists? dumb
            let _ = fs::create_dir_all(parent);
        }

        trace!("writing file {:?}", to);

        File::create(&to)
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
        Regex::new(
            concat!(
                "(?ms)",
                r"\A---\s*\n",
                r"(?P<metadata>.*?\n?)",
                r"^---\s*$",
                r"\n*",
                r"(?P<body>.*)"))
            .unwrap();

    let body = if let Some(captures) = re.captures(&item.body) {
        if let Some(metadata) = captures.name("metadata") {
            if let Ok(parsed) = metadata.parse() {
                item.extensions.insert(Metadata { data: parsed });
            }
        }

        captures.name("body").map(|b| b.to_string())
    } else { None };

    if let Some(body) = body {
        item.body = body;
    }

    Ok(())
}

pub fn markdown(extensions: hoedown::Extension, use_smartypants: bool) -> Markdown {
    Markdown {
        extensions: extensions,
        use_smartypants: use_smartypants,
    }
}

pub struct Markdown {
    extensions: hoedown::Extension,
    use_smartypants: bool,
}

impl Handle<Item> for Markdown {
    fn handle(&self, item: &mut Item) -> handle::Result {
        let document =
            hoedown::Markdown::new(item.body.as_bytes())
            .with_extensions(self.extensions.clone());

        let renderer = html::Html::new(html::Flags::empty(), 0);
        let mut buffer = document.render_to_buffer(renderer);

        if self.use_smartypants {
            let mut smartypants = hoedown::buffer::Buffer::new(64);
            html::smartypants(&mut smartypants, &mut buffer);

            item.body = smartypants.as_str().unwrap().to_string();
        } else {
            item.body = buffer.as_str().unwrap().to_string();
        }

        Ok(())
    }
}

pub fn is_draft(item: &Item) -> bool {
    item.extensions.get::<Metadata>()
        .map(|meta| {
            meta.data.lookup("draft")
                .and_then(::toml::Value::as_bool)
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

pub fn publishable(item: &Item) -> bool {
    !(is_draft(item) && !item.bind().configuration.is_preview)
}

#[derive(Clone)]
pub struct Page {
    pub first: (usize, Arc<PathBuf>),
    pub next: Option<(usize, Arc<PathBuf>)>,
    pub curr: (usize, Arc<PathBuf>),
    pub prev: Option<(usize, Arc<PathBuf>)>,
    pub last: (usize, Arc<PathBuf>),

    pub range: Range<usize>,

    pub page_count: usize,
    pub post_count: usize,
    pub posts_per_page: usize,
}

