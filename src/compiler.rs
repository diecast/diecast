//! Compiler behavior.

use std::sync::Arc;
use toml;

use item::{Item, Dependencies};

/// Behavior of a compiler.
///
/// There's a single method that takes a mutable
/// reference to the `Item` being compiled.
pub trait Compile: Send + Sync {
    fn compile(&self, item: &mut Item, dependencies: Option<Dependencies>);
}

impl<T: ?Sized + Compile> Compile for Box<T> {
    fn compile(&self, item: &mut Item, dependencies: Option<Dependencies>) {
        (**self).compile(item, dependencies);
    }
}

impl<T: ?Sized + Compile> Compile for &'static T {
    fn compile(&self, item: &mut Item, dependencies: Option<Dependencies>) {
        (**self).compile(item, dependencies);
    }
}

impl<T: ?Sized + Compile> Compile for &'static mut T {
    fn compile(&self, item: &mut Item, dependencies: Option<Dependencies>) {
        (**self).compile(item, dependencies);
    }
}



impl<F> Compile for F where F: Fn(&mut Item, Option<Dependencies>) + Send + Sync {
    fn compile(&self, item: &mut Item, deps: Option<Dependencies>) {
        self(item, deps);
    }
}

pub enum Link {
    Compiler(Box<Compile + Send + Sync>),
    Barrier,
}

#[derive(Clone, Copy)]
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
    where C: Compile {
        Chain {
            chain: vec![Link::Compiler(Box::new(compiler) as Box<Compile + Send + Sync>)]
        }
    }

    pub fn link<C>(mut self, compiler: C) -> Chain
    where C: Compile {
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
            status: self.status.clone(),
            position: self.position.clone(),
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

    pub fn compile(&mut self, item: &mut Item, deps: Option<Dependencies>) {
        for link in &self.chain[self.position..] {
            self.position += 1;

            match *link {
                Link::Compiler(ref compiler) => compiler.compile(item, deps.clone()),
                Link::Barrier => {
                    self.status = Status::Paused;
                    return;
                },
            }
        }

        self.status = Status::Done;
    }
}

pub fn stub(item: &mut Item, _deps: Option<Dependencies>) {
    trace!("no compiler established for: {:?}", item);
}

/// Compiler that reads the `Item`'s body.
pub fn read(item: &mut Item, _deps: Option<Dependencies>) {
    item.read();
}

/// Compiler that writes the `Item`'s body.
pub fn write(item: &mut Item, _deps: Option<Dependencies>) {
    item.write();
}

/// Compiler that prints the `Item`'s body.
pub fn print(item: &mut Item, _deps: Option<Dependencies>) {
    use std::old_io::stdio::println;

    if let &Some(ref body) = &item.body {
        println(body);
    } else {
        println("no body");
    }
}

#[derive(Clone)]
pub struct Metadata(pub String);

pub fn parse_metadata(item: &mut Item, _deps: Option<Dependencies>) {
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

pub fn parse_toml(item: &mut Item, _deps: Option<Dependencies>) {
    let parsed = if let Some(&Metadata(ref parsed)) = item.data.get::<Metadata>() {
        Some(toml::Value::Table(toml::Parser::new(parsed).parse().unwrap()))
    } else {
        None
    };

    if let Some(parsed) = parsed {
        item.data.insert(TomlMetadata(parsed));
    }
}

pub fn render_markdown(item: &mut Item, _deps: Option<Dependencies>) {
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
    fn compile(&self, item: &mut Item, _deps: Option<Dependencies>) {
        item.data.insert(self.data.clone());
    }
}

use rustc_serialize::json::Json;
use handlebars::Handlebars;

pub struct RenderTemplate {
    name: &'static str,
    handler: Box<Fn(&Item, Option<Dependencies>) -> Json + Sync + Send + 'static>,
}

impl RenderTemplate {
    pub fn new<H>(name: &'static str, handler: H) -> RenderTemplate
    where H: Fn(&Item, Option<Dependencies>) -> Json + Sync + Send + 'static {
        RenderTemplate {
            name: name,
            handler: Box::new(handler),
        }
    }
}

impl Compile for RenderTemplate {
    fn compile(&self, item: &mut Item, deps: Option<Dependencies>) {
        if let Some(ref registry) = item.data.get::<Arc<Handlebars>>() {
            let json = (*self.handler)(item, deps);
            item.body = Some(registry.render(self.name, &json).unwrap());
        }
    }
}

