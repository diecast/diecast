//! item::Handler behavior.

use std::sync::Arc;
use std::sync::mpsc::channel;
use std::error::FromError;
use std::path::PathBuf;

use toml;

use job;
use item::{self, Item};
use binding::{self, Bind};
use compiler;
use threadpool::ThreadPool;

pub trait Error: ::std::error::Error {}
pub type Result = ::std::result::Result<(), Box<Error>>;

impl<E> Error for E where E: ::std::error::Error {}

impl<E> FromError<E> for Box<Error> where E: Error + 'static {
    fn from_error(e: E) -> Box<Error> {
        Box::new(e)
    }
}

pub enum Kind {
    Fork(Arc<Box<item::Handler + Sync + Send>>),
    Join(Box<binding::Handler + Sync + Send>),
}

// TODO
// rename Compiler -> Hybrid
// have Job contain a Box<binding::Handler> ?
pub struct Compiler {
    handlers: Vec<Kind>,
}

impl Compiler {
    pub fn new() -> Compiler {
        Compiler {
            handlers: vec![],
        }
    }

    pub fn fork<H>(mut self, compiler: H) -> Compiler
    where H: item::Handler + Sync + Send + 'static {
        self.handlers.push(Kind::Fork(Arc::new(Box::new(compiler))));
        self
    }

    pub fn join<H>(mut self, compiler: H) -> Compiler
    where H: binding::Handler + Sync + Send + 'static {
        self.handlers.push(Kind::Join(Box::new(compiler)));
        self
    }
}

// TODO
// this is pretty confusing because the Job is scheduled to the threadpool
// but then the handling of this Compiler itself requires a threadpool scheduling
// for the purpose of the fork handlers
//
// LEVELS OF GRANULARITY
// binding: jobs contain full bindings. jobs are enqueued into thread pool and dequeued out
impl binding::Handler for Compiler {
    fn handle(&self, binding: &mut Bind) -> compiler::Result {
        trace!("performing Compiler::handler which has {} handlers", self.handlers.len());

        // TODO
        // add configuration to binding? must also be on item
        // FIXME
        // this ends up creating a threadpool for EVERY instance of a compiler
        let pool = ThreadPool::new(4);
        let (tx, rx) = channel();

        for (idx, kind) in self.handlers.iter().enumerate() {
            trace!("processing handler {}", idx);

            match *kind {
                Kind::Fork(ref handler) => {
                    let count = binding.items.len();

                    // TODO
                    // use chunks to send a chunks to each thread instead of just one?
                    for item in binding.items.drain() {
                        let handler = handler.clone();
                        let tx = tx.clone();

                        pool.execute(move || {
                            let mut item = item;

                            match handler.handle(&mut item) {
                                Ok(()) => {
                                    tx.send(Ok(item)).unwrap()
                                },
                                Err(e) => {
                                    println!("\nthe following item encountered an error:\n  {:?}\n\n{}\n", item, e);
                                    tx.send(Err(job::Error::Err)).unwrap();
                                }
                            }
                        });
                    }

                    // fork processing is not stable
                    // i.e. order is not preserved
                    for _ in 0 .. count {
                        match rx.recv().unwrap() {
                            Ok(item) => {
                                binding.items.push(item);
                            },
                            Err(job::Error::Err) => {
                                println!("an item returned an error. stopping everything");
                                ::exit(1);
                            },
                            Err(job::Error::Panic) => {
                                println!("an item panicked. stopping everything");
                                ::exit(1);
                            }
                        }
                    }
                },
                Kind::Join(ref handler) => {
                    try!(handler.handle(binding));
                },
            }
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
        Some(parsed.parse().unwrap())
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

#[derive(Clone)]
pub struct Pagination {
    pub first_number: usize,
    pub first_path: PathBuf,

    pub last_number: usize,
    pub last_path: PathBuf,

    pub next_number: Option<usize>,
    pub next_path: Option<PathBuf>,

    pub curr_number: usize,
    pub curr_path: PathBuf,

    pub prev_number: Option<usize>,
    pub prev_path: Option<PathBuf>,

    pub page_count: usize,
    pub post_count: usize,
    pub posts_per_page: usize,
}

