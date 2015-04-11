//! item::Handler behavior.

use std::sync::Arc;
use std::path::PathBuf;
use std::collections::HashMap;
use std::ops::Range;
use std::fs::PathExt;
use std::fs;
use std::path::Path;
use std::any::Any;

use toml;

use compiler;
use item::{self, Item, Route};
use binding::{self, Bind};
use pattern::Pattern;
use job::evaluator::Pool;

pub trait Error: ::std::error::Error {}
pub type Result = ::std::result::Result<(), Box<Error>>;

impl<E> Error for E where E: ::std::error::Error {}

impl<E> From<E> for Box<Error> where E: Error + 'static {
    fn from(e: E) -> Box<Error> {
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
        for handler in &self.handlers {
            try!(handler.handle(binding));
        }

        Ok(())
    }
}

// TODO: should the chunk be in configuration or a parameter?
pub struct Pooled<H>
where H: item::Handler + Sync + Send + 'static {
    chunk: usize,
    handler: Arc<H>,
}

impl<H> Pooled<H>
where H: item::Handler + Sync + Send + 'static {
    pub fn new(handler: H) -> Pooled<H> {
        Pooled {
            chunk: 1,
            handler: Arc::new(handler),
        }
    }

    pub fn chunk(mut self, size: usize) -> Pooled<H> {
        self.chunk = size;
        self
    }
}

impl<H> binding::Handler for Pooled<H>
where H: item::Handler + Sync + Send + 'static {
    fn handle(&self, bind: &mut Bind) -> compiler::Result {
        let pool: Pool<Vec<Item>> = Pool::new(bind.data().configuration.threads);
        let item_count = bind.items.len();

        let chunks = {
            let (div, rem) = (item_count / self.chunk, item_count % self.chunk);

            if rem == 0 {
                div
            } else {
                div + 1
            }
        };

        let mut items = bind.items.drain().collect::<Vec<Item>>();

        // TODO: optimize this for general case of chunk=1?
        while !items.is_empty() {
            let rest = if self.chunk > items.len() {
                vec![]
            } else {
                items.split_off(self.chunk)
            };

            let handler = self.handler.clone();

            pool.enqueue(move || {
                let mut results = vec![];

                for mut item in items {
                    match item::Handler::handle(&handler, &mut item) {
                        Ok(()) => results.push(item),
                        Err(e) => {
                            println!("\nthe following item encountered an error:\n  {:?}\n\n{}\n", item, e);
                            return None;
                        }
                    }
                }

                Some(results)
            });

            items = rest;
        }

        for _ in 0 .. chunks {
            bind.items.extend(pool.dequeue().unwrap().into_iter());
        }

        assert!(item_count == bind.items.len(), "received different number of items from pool");

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
        for handler in &self.handlers {
            try!(handler.handle(item));
        }

        Ok(())
    }
}

impl binding::Handler for ItemChain {
    fn handle(&self, binding: &mut Bind) -> compiler::Result {
        for item in &mut binding.items {
            try!(item::Handler::handle(self, item));
        }

        Ok(())
    }
}

pub fn stub(_bind: &mut Bind) -> Result {
    trace!("stub compiler");
    Ok(())
}

/// item::Handler that reads the `Item`'s body.
pub fn read(item: &mut Item) -> Result {
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

/// item::Handler that writes the `Item`'s body.
pub fn write(item: &mut Item) -> Result {
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


/// item::Handler that prints the `Item`'s body.
pub fn print(item: &mut Item) -> Result {
    println!("{}", item.body);

    Ok(())
}

#[derive(Clone)]
pub struct Metadata {
    pub data: toml::Value,
}

pub fn parse_metadata(item: &mut Item) -> Result {
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

pub fn render_markdown(item: &mut Item) -> Result {
    use hoedown::Markdown;
    use hoedown::renderer::html;

    let document = Markdown::new(item.body.as_bytes());
    let renderer = html::Html::new(html::Flags::empty(), 0);
    let buffer = document.render_to_buffer(renderer);
    item.data.insert(buffer);

    Ok(())
}

pub fn inject_item_data<T>(t: Arc<T>) -> Box<item::Handler + Sync + Send>
where T: Any + Sync + Send + 'static {
    Box::new(move |item: &mut Item| -> Result {
        item.data.insert(t.clone());
        Ok(())
    })
}

pub fn inject_bind_data<T>(t: Arc<T>) -> Box<binding::Handler + Sync + Send>
where T: Any + Sync + Send + 'static {
    Box::new(move |bind: &mut Bind| -> Result {
        bind.data().data.write().unwrap().insert(t.clone());
        Ok(())
    })
}

use rustc_serialize::json::Json;
use handlebars::Handlebars;

pub fn render_template<H>(name: &'static str, handler: H)
    -> Box<item::Handler + Sync + Send>
where H: Fn(&Item) -> Json + Sync + Send + 'static {
    Box::new(move |item: &mut Item| -> Result {
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

#[derive(Clone, Debug)]
pub struct Adjacent {
    previous: Option<Arc<Item>>,
    next: Option<Arc<Item>>,
}

pub fn next_prev(bind: &mut Bind) -> compiler::Result {
    let count = bind.items.len();

    let last_num = if count == 0 {
        0
    } else {
        count - 1
    };

    // TODO: yet another reason to have Arc<Item>?
    let cloned = bind.items.iter().map(|i| Arc::new(i.clone())).collect::<Vec<Arc<Item>>>();

    for (idx, item) in bind.items.iter_mut().enumerate() {
        let prev =
            if idx == 0 { None }
            else { let num = idx - 1; Some(cloned[num].clone()) };
        let next =
            if idx == last_num { None }
            else { let num = idx + 1; Some(cloned[num].clone()) };

        item.data.insert::<Adjacent>(Adjacent {
            previous: prev,
            next: next,
        });
    }

    Ok(())
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

pub fn paginate<R>(factor: usize, router: R) -> Box<binding::Handler + Sync + Send>
where R: Fn(usize) -> PathBuf, R: Sync + Send + 'static {
    Box::new(move |bind: &mut Bind| -> compiler::Result {
        let post_count = bind.data().dependencies["posts"].items.len();

        let page_count = {
            let (div, rem) = (post_count / factor, post_count % factor);

            if rem == 0 {
                div
            } else {
                div + 1
            }
        };

        let last_num = page_count - 1;

        let mut cache: HashMap<usize, Arc<PathBuf>> = HashMap::new();

        let mut router = |num: usize| -> Arc<PathBuf> {
            cache.entry(num)
                .or_insert_with(|| Arc::new(router(num)))
                .clone()
        };

        let first = (1, router(1));
        let last = (last_num, router(last_num));

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

            let target = router(current);

            let first = first.clone();
            let last = last.clone();
            let curr = (current, target.clone());

            let page_struct =
                Page {
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

            let page = bind.new_item(Route::Write((*target).clone()));
            page.data.insert::<Page>(page_struct);
        }

        println!("finished pagination");

        Ok(())
    })
}

// TODO: problem here is that the dir is being walked multiple times
pub fn from_pattern<P>(pattern: P) -> Box<binding::Handler + Sync + Send>
where P: Pattern + Sync + Send + 'static {
    Box::new(move |bind: &mut Bind| -> compiler::Result {
        let paths =
            fs::walk_dir(&bind.data().configuration.input).unwrap()
            .filter_map(|p| {
                let path = p.unwrap().path();

                if let Some(ref pattern) = bind.data().configuration.ignore {
                    if pattern.matches(&Path::new(path.file_name().unwrap())) {
                        return None;
                    }
                }

                if path.is_file() {
                    Some(path.to_path_buf())
                } else {
                    None
                }
            })
            .collect::<Vec<PathBuf>>();

        for path in &paths {
            let relative =
                path.relative_from(&bind.data().configuration.input).unwrap()
                .to_path_buf();

            if pattern.matches(&relative) {
                bind.new_item(Route::Read(relative));
            }
        }

        Ok(())
    })
}

pub fn creating(path: PathBuf) -> Box<binding::Handler + Sync + Send> {
    Box::new(move |bind: &mut Bind| -> compiler::Result {
        bind.new_item(Route::Write(path.clone()));

        Ok(())
    })
}

#[derive(Clone)]
pub struct Tags {
    map: HashMap<String, Vec<Arc<Item>>>,
}

pub fn tags(bind: &mut Bind) -> compiler::Result {
    let mut tag_map = ::std::collections::HashMap::new();

    for item in &bind.items {
        let toml =
            item.data.get::<Metadata>()
            .and_then(|m| {
                m.data.lookup("tags")
            })
            .and_then(::toml::Value::as_slice);

        let arc = Arc::new(item.clone());

        if let Some(tags) = toml {
            for tag in tags {
                tag_map.entry(tag.as_str().unwrap().to_string())
                    .or_insert(vec![])
                    .push(arc.clone());
            }
        }
    }

    bind.data().data.write().unwrap().insert::<Tags>(Tags { map: tag_map });

    Ok(())
}
