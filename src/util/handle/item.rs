use std::sync::Arc;
use std::path::PathBuf;
use std::ops::Range;
use std::any::Any;
use std::collections::HashMap;

use regex::Regex;
use toml;
use chrono;
use typemap;

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
where T: typemap::Key, T::Value: Any + Sync + Send + Clone {
    fn handle(&self, item: &mut Item) -> handle::Result {
        item.extensions.insert::<T>(self.payload.clone());
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
                ::mkdir_p(parent).unwrap();
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
    use std::fs::File;
    use std::io::Write;

    if let Some(to) = item.target() {
        // TODO: once path normalization is in, make sure
        // writing to output folder
        if let Some(parent) = to.parent() {
            // TODO: this errors out if the path already exists? dumb
            ::mkdir_p(parent).unwrap();
        }

        trace!("writing file {:?}", to);

        // TODO: this sometimes crashes
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

pub struct Metadata;

impl typemap::Key for Metadata {
    type Value = toml::Value;
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
                item.extensions.insert::<Metadata>(parsed);
            }
        }

        captures.name("body").map(String::from)
    } else { None };

    if let Some(body) = body {
        item.body = body;
    }

    Ok(())
}

pub fn is_draft(item: &Item) -> bool {
    item.extensions.get::<Metadata>()
        .map(|meta| {
            meta.lookup("draft")
                .and_then(::toml::Value::as_bool)
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

pub fn publishable(item: &Item) -> bool {
    !(is_draft(item) && !item.bind().configuration.is_preview)
}

// TODO: should this just contain the items itself instead of the range?
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

impl typemap::Key for Page {
    type Value = Page;
}

// TODO: audit; perhaps have Item::versions
pub struct Versions;

impl typemap::Key for Versions {
    type Value = HashMap<String, String>;
}

pub struct SaveVersion {
    name: String,
}

impl Handle<Item> for SaveVersion {
    fn handle(&self, item: &mut Item) -> handle::Result {
        item.extensions.entry::<Versions>()
            .or_insert_with(|| HashMap::new())
            .insert(self.name.clone(), item.body.clone());

        Ok(())
    }
}

pub fn save_version<S: Into<String>>(name: S) -> SaveVersion {
    SaveVersion {
        name: name.into()
    }
}

pub struct Date;

impl typemap::Key for Date {
    type Value = chrono::NaiveDate;
}

// TODO
// * make time type generic
// * customizable format
pub fn date(item: &mut Item) -> handle::Result {
    let date = {
        if let Some(meta) = item.extensions.get::<Metadata>() {
            let date = meta.lookup("published").and_then(toml::Value::as_str).unwrap();

            Some(chrono::NaiveDate::parse_from_str(date, "%B %e, %Y").unwrap())
        } else {
            None
        }
    };

    if let Some(date) = date {
        item.extensions.insert::<Date>(date);
    }

    Ok(())
}

pub struct HandleIf<C, H>
where C: Fn(&Item) -> bool, C: Sync + Send + 'static,
      H: Handle<Item> + Sync + Send + 'static {
    condition: C,
    handler: H,
}

impl<C, H> Handle<Item> for HandleIf<C, H>
where C: Fn(&Item) -> bool, C: Sync + Send + 'static,
      H: Handle<Item> + Sync + Send + 'static {
    fn handle(&self, item: &mut Item) -> handle::Result {
        if (self.condition)(item) {
            (self.handler.handle(item))
        } else {
            Ok(())
        }
    }
}

#[inline]
pub fn handle_if<C, H>(condition: C, handler: H) -> HandleIf<C, H>
where C: Fn(&Item) -> bool, C: Copy + Sync + Send + 'static,
      H: Handle<Item> + Sync + Send + 'static {
    HandleIf {
        condition: condition,
        handler: handler,
    }
}

