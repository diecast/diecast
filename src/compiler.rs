//! Compiler behavior.

use std::sync::Arc;
use toml;

use item::Item;

/// Behavior of a compiler.
///
/// There's a single method that takes a mutable
/// reference to the `Item` being compiled.
pub trait Compile: Send + Sync {
    fn compile(&self, item: &mut Item);
}

impl<C: ?Sized> Compile for Box<C> where C: Compile {
    fn compile(&self, item: &mut Item) {
        (**self).compile(item);
    }
}

impl<C: ?Sized> Compile for &'static C where C: Compile {
    fn compile(&self, item: &mut Item) {
        (**self).compile(item);
    }
}

impl<C: ?Sized> Compile for &'static mut C where C: Compile {
    fn compile(&self, item: &mut Item) {
        (**self).compile(item);
    }
}

impl<F> Compile for F where F: Fn(&mut Item) + Send + Sync {
    fn compile(&self, item: &mut Item) {
        self(item);
    }
}

pub enum Link {
    Compiler(Box<Compile + Send + Sync>),
    Barrier,
}

#[derive(Copy)]
pub enum Status {
    Paused,
    Done,
}

pub struct Chain {
    chain: Vec<Link>,
}

impl Chain {
    pub fn new() -> Chain {
        Chain { chain: Vec::new() }
    }

    pub fn only<C>(compiler: C) -> Chain
    where C: Compile + 'static {
        Chain {
            chain: vec![Link::Compiler(Box::new(compiler) as Box<Compile + Send + Sync>)]
        }
    }

    pub fn link<C>(mut self, compiler: C) -> Chain
    where C: Compile + 'static {
        self.chain.push(Link::Compiler(Box::new(compiler) as Box<Compile + Send + Sync>));
        self
    }

    pub fn barrier(mut self) -> Chain {
        self.chain.push(Link::Barrier);
        self
    }

    pub fn build(self) -> Arc<Vec<Link>> {
        Arc::new(self.chain)
    }
}

/// Chain of compilers.
///
/// Maintains a list of compilers and executes them
/// in the order they were added.
pub struct Compiler {
    pub chain: Arc<Vec<Link>>,
    pub status: Status,
    position: usize,
}

impl Clone for Compiler {
    fn clone(&self) -> Compiler {
        Compiler {
            chain: self.chain.clone(),
            status: self.status,
            position: self.position,
        }
    }
}

impl Compiler {
    pub fn new(chain: Arc<Vec<Link>>) -> Compiler {
        Compiler {
            chain: chain,
            position: 0,
            status: Status::Paused,
        }
    }

    pub fn compile(&mut self, item: &mut Item) {
        for link in &self.chain[self.position..] {
            self.position += 1;

            match *link {
                Link::Compiler(ref compiler) => compiler.compile(item),
                Link::Barrier => {
                    self.status = Status::Paused;
                    return;
                },
            }
        }

        self.status = Status::Done;
    }
}

pub fn stub(item: &mut Item) {
    trace!("no compiler established for: {:?}", item);
}

/// Compiler that reads the `Item`'s body.
pub fn read(item: &mut Item) {
    use std::fs::File;
    use std::io::{Read, Write};

    if let Some(ref path) = item.from {
        let mut buf = String::new();

        File::open(&item.configuration.input.join(path))
            .unwrap()
            .read_to_string(&mut buf)
            .unwrap();

        item.body = Some(buf);
    }
}

/// Compiler that writes the `Item`'s body.
pub fn write(item: &mut Item) {
    use std::fs::{self, File};
    use std::io::Write;

    if let Some(ref path) = item.to {
        if let Some(ref body) = item.body {
            let conf_out = &item.configuration.output;
            let target = conf_out.join(path.parent().unwrap());

            if !target.starts_with(&conf_out) {
                panic!("attempted to write outside of the output directory: {:?}", target);
            }

            trace!("mkdir -p {:?}", target);

            // TODO: this errors out if the path already exists? dumb
            fs::create_dir_all(&target);

            File::create(&conf_out.join(path))
                .unwrap()
                .write_all(body.as_bytes())
                .unwrap();
        }
    }
}


/// Compiler that prints the `Item`'s body.
pub fn print(item: &mut Item) {
    use std::old_io::stdio::println;

    if let &Some(ref body) = &item.body {
        println(body);
    } else {
        println("no body");
    }
}

#[derive(Clone)]
pub struct Metadata(pub String);

pub fn parse_metadata(item: &mut Item) {
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
                return;
            } else {
                item.body = None;
                return;
            }
        }

        item.body = Some(body);
    }
}

#[derive(Clone)]
pub struct TomlMetadata(pub toml::Value);

pub fn parse_toml(item: &mut Item) {
    let parsed = if let Some(&Metadata(ref parsed)) = item.data.get::<Metadata>() {
        Some(parsed.parse().unwrap())
    } else {
        None
    };

    if let Some(parsed) = parsed {
        item.data.insert(TomlMetadata(parsed));
    }
}

pub fn render_markdown(item: &mut Item) {
    use hoedown::Markdown;
    use hoedown::renderer::html;

    if let Some(body) = item.body.take() {
        let document = Markdown::new(body.as_bytes());
        let renderer = html::Html::new(html::Flags::empty(), 0);
        let buffer = document.render_to_buffer(renderer);
        item.data.insert(buffer);
        item.body = Some(body);
    }
}

pub fn inject_with<T>(t: Arc<T>) -> Box<Compile + Sync + Send>
where T: Sync + Send + 'static {
    Box::new(move |item: &mut Item| {
        item.data.insert(t.clone());
    })
}

// TODO: this isn't necessary. in fact, on Barriers, it ends up cloning the data
// this is mainly to ensure that the user is passing an Arc
#[derive(Clone)]
pub struct Inject<T> where T: Sync + Send + 'static {
    data: Arc<T>,
}

impl<T> Inject<T> where T: Sync + Send + 'static {
    pub fn with(t: Arc<T>) -> Inject<T>
    where T: Sync + Send + 'static {
        Inject {
            data: t
        }
    }
}

impl<T> Compile for Inject<T> where T: Sync + Send + 'static {
    fn compile(&self, item: &mut Item) {
        item.data.insert(self.data.clone());
    }
}

use rustc_serialize::json::Json;
use handlebars::Handlebars;

pub fn render_template<H>(name: &'static str, handler: H) -> Box<Compile + Sync + Send>
where H: Fn(&Item) -> Json + Sync + Send + 'static {
    Box::new(move |item: &mut Item| {
        if let Some(ref registry) = item.data.get::<Arc<Handlebars>>() {
            let json = handler(item);
            item.body = Some(registry.render(name, &json).unwrap());
        }
    })
}

pub struct RenderTemplate {
    name: &'static str,
    handler: Box<Fn(&Item) -> Json + Sync + Send + 'static>,
}

impl RenderTemplate {
    pub fn new<H>(name: &'static str, handler: H) -> RenderTemplate
    where H: Fn(&Item) -> Json + Sync + Send + 'static {
        RenderTemplate {
            name: name,
            handler: Box::new(handler),
        }
    }
}

impl Compile for RenderTemplate {
    fn compile(&self, item: &mut Item) {
        if let Some(ref registry) = item.data.get::<Arc<Handlebars>>() {
            let json = (*self.handler)(item);
            item.body = Some(registry.render(self.name, &json).unwrap());
        }
    }
}

