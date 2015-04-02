//! item::Handler behavior.

use std::sync::Arc;
use std::error::FromError;
use std::path::PathBuf;
use std::collections::HashSet;

use toml;

use compiler;
use rule::Rule;
use item::{self, Item};
use binding::{self, Bind};

pub trait Error: ::std::error::Error {}
pub type Result = ::std::result::Result<(), Box<Error>>;

impl<E> Error for E where E: ::std::error::Error {}

impl<E> FromError<E> for Box<Error> where E: Error + 'static {
    fn from_error(e: E) -> Box<Error> {
        Box::new(e)
    }
}

pub struct BindChain {
    handlers: Vec<Box<binding::Handler + Sync + Send>>,
}

impl BindChain {
    pub fn new() -> BindChain {
        BindChain {
            handlers: vec![],
        }
    }

    pub fn link<H>(mut self, compiler: H) -> BindChain
    where H: binding::Handler + Sync + Send + 'static {
        self.handlers.push(Box::new(compiler));
        self
    }
}

impl binding::Handler for BindChain {
    fn handle(&self, binding: &mut Bind) -> compiler::Result {
        trace!("performing BindChain::handler which has {} handlers", self.handlers.len());

        for handler in &self.handlers {
            try!(handler.handle(binding));
        }

        Ok(())
    }
}

pub struct ItemChain {
    handlers: Vec<Box<item::Handler + Sync + Send>>,
}

impl ItemChain {
    pub fn new() -> ItemChain {
        ItemChain {
            handlers: vec![],
        }
    }

    pub fn link<H>(mut self, compiler: H) -> ItemChain
    where H: item::Handler + Sync + Send + 'static {
        self.handlers.push(Box::new(compiler));
        self
    }
}

impl item::Handler for ItemChain {
    fn handle(&self, item: &mut Item) -> compiler::Result {
        trace!("performing ItemChain::handler which has {} handlers", self.handlers.len());

        for handler in &self.handlers {
            try!(handler.handle(item));
        }

        Ok(())
    }
}

impl binding::Handler for ItemChain {
    fn handle(&self, binding: &mut Bind) -> compiler::Result {
        trace!("performing ItemChain::handler which has {} handlers", self.handlers.len());

        for item in &mut binding.items {
            try!(item::Handler::handle(self, item));
        }

        Ok(())
    }
}

pub fn stub(item: &mut Item) -> Result {
    trace!("no compiler established for: {:?}", item);
    Ok(())
}

/// item::Handler that reads the `Item`'s body.
pub fn read(item: &mut Item) -> Result {
    use std::fs::File;
    use std::io::Read;

    if let Some(ref path) = item.from {
        let mut buf = String::new();

        // TODO: use try!
        File::open(&item.bind().configuration.input.join(path))
            .unwrap()
            .read_to_string(&mut buf)
            .unwrap();

        item.body = Some(buf);
    }

    Ok(())
}

/// item::Handler that writes the `Item`'s body.
pub fn write(item: &mut Item) -> Result {
    use std::fs::{self, File};
    use std::io::Write;

    if let Some(ref path) = item.to {
        if let Some(ref body) = item.body {
            let conf_out = &item.bind().configuration.output;
            let target = conf_out.join(path);

            if !target.starts_with(&conf_out) {
                // TODO
                // should probably return a proper T: Error?
                println!("attempted to write outside of the output directory: {:?}", target);
                ::exit(1);
            }

            if let Some(parent) = target.parent() {
                trace!("mkdir -p {:?}", parent);

                // TODO: this errors out if the path already exists? dumb
                let _ = fs::create_dir_all(parent);
            }

            let file = conf_out.join(path);

            trace!("writing file {:?}", file);

            File::create(&file)
                .unwrap()
                .write_all(body.as_bytes())
                .unwrap();
        }
    }

    Ok(())
}


/// item::Handler that prints the `Item`'s body.
pub fn print(item: &mut Item) -> Result {
    if let &Some(ref body) = &item.body {
        println!("{}", body);
    }

    Ok(())
}

#[derive(Clone)]
pub struct Metadata(pub String);

pub fn parse_metadata(item: &mut Item) -> Result {
    if let Some(body) = item.body.take() {
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
                    r"\n?",
                    r"(?P<body>.*)"));

        if let Some(captures) = re.captures(&body) {
            if let Some(metadata) = captures.name("metadata") {
                item.data.insert(Metadata(metadata.to_string()));
            }

            if let Some(body) = captures.name("body") {
                item.body = Some(body.to_string());
                return Ok(());
            } else {
                item.body = None;
                return Ok(());
            }
        }

        item.body = Some(body);
    }

    Ok(())
}

#[derive(Clone)]
pub struct TomlMetadata(pub toml::Value);

pub fn parse_toml(item: &mut Item) -> Result {
    let parsed = if let Some(&Metadata(ref parsed)) = item.data.get::<Metadata>() {
        // TODO: proper error handling here
        parsed.parse().ok()
    } else {
        None
    };

    if let Some(parsed) = parsed {
        item.data.insert(TomlMetadata(parsed));
    }

    Ok(())
}

pub fn render_markdown(item: &mut Item) -> Result {
    use hoedown::Markdown;
    use hoedown::renderer::html;

    if let Some(body) = item.body.take() {
        let document = Markdown::new(body.as_bytes());
        let renderer = html::Html::new(html::Flags::empty(), 0);
        let buffer = document.render_to_buffer(renderer);
        item.data.insert(buffer);
        item.body = Some(body);
    }

    Ok(())
}

pub fn inject_with<T>(t: Arc<T>) -> Box<item::Handler + Sync + Send>
where T: Sync + Send + 'static {
    Box::new(move |item: &mut Item| -> Result {
        item.data.insert(t.clone());
        Ok(())
    })
}

use rustc_serialize::json::Json;
use handlebars::Handlebars;

pub fn render_template<H>(name: &'static str, handler: H) -> Box<item::Handler + Sync + Send>
where H: Fn(&Item) -> Json + Sync + Send + 'static {
    Box::new(move |item: &mut Item| -> Result {
        if let Some(ref registry) = item.data.get::<Arc<Handlebars>>() {
            let json = handler(item);
            item.body = Some(registry.render(name, &json).unwrap());
        }

        Ok(())
    })
}

// TODO: this needs Copy so it can be 'moved' to the retain method more than once
// even if we're not actually doing it more than once
// in general this means that it can only be used with a function
// perhaps should make the bound be Clone once Copy: Clone is implemented
pub fn retain<C>(condition: C) -> Box<binding::Handler + Sync + Send>
where C: Fn(&Item) -> bool, C: Copy + Sync + Send + 'static {
    Box::new(move |bind: &mut Bind| -> Result {
        bind.items.retain(condition);
        Ok(())
    })
}

#[derive(Clone)]
pub struct Pagination {
    pub first: (usize, PathBuf),
    pub last: (usize, PathBuf),
    pub next: Option<(usize, PathBuf)>,
    pub curr: (usize, PathBuf),
    pub prev: Option<(usize, PathBuf)>,

    pub range: ::std::ops::Range<usize>,

    pub page_count: usize,
    pub post_count: usize,
    pub posts_per_page: usize,
}

pub fn paginate<R>(factor: usize, router: R) -> Box<binding::Handler + Sync + Send>
where R: Fn(usize) -> PathBuf, R: Sync + Send + 'static {
    Box::new(move |bind: &mut Bind| -> compiler::Result {
        println!("starting paginate");
        let posts = &bind.data.read().unwrap().dependencies["posts"].items;

        // TODO
        // test:
        //   1. no items
        //   2. just one item

        let post_count = posts.len();

        // this conversion could fail if post_count or factor is greater than 2^53
        // I think that's unlikely...
        let page_count = ::std::cmp::max(1, (post_count as f64 / factor as f64).ceil() as usize);

        let last_num = ::std::cmp::max(1, page_count - 1);

        let first = (1, router(1));
        let last = (last_num, router(last_num));

        // TODO: this seems hacky
        // perhaps move matching/creating behavior to compiler
        bind.items.clear();

        // grow the number of pages as needed
        for current in 0 .. page_count {
            let prev =
                if current == 0 { None }
                else { let num = current - 1; Some((num, router(num))) };
            let next =
                if current == last_num { None }
                else { let num = current + 1; Some((num, router(num))) };

            let start = current * factor;
            let end = ::std::cmp::min(post_count, (current + 1) * factor);

            println!("page {} has a range of [{}, {})", current, start, end);

            let first = first.clone();
            let last = last.clone();
            let curr = (current, router(current));

            let page_struct =
                Pagination {
                    first: first,

                    prev: prev,
                    curr: curr,
                    next: next,

                    last: last,

                    page_count: page_count,
                    post_count: post_count,
                    posts_per_page: factor,

                    range: start .. end,
                };

            let mut page = Item::to(PathBuf::from(format!("{}/index.html", current)), bind.data.clone());
            page.data.insert::<Pagination>(page_struct);
            bind.items.push(page);
        }

        Ok(())
    })
}

