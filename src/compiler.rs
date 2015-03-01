//! Compiler behavior.

use std::sync::Arc;
use std::cell::Cell;
use toml;

use item::Item;

/// Behavior of a compiler.
///
/// There's a single method that takes a mutable
/// reference to the `Item` being compiled.
pub trait Compile: Send + Sync {
    fn compile(&self, item: &mut Item);
}

pub trait Cloner {
    fn clone_compiler(&self) -> Box<ClonableCompile + Send + Sync>;
}

impl<T> Cloner for T where T: Compile + Send + Sync + Clone + 'static {
    fn clone_compiler(&self) -> Box<ClonableCompile + Send + Sync> {
        Box::new(self.clone())
    }
}

pub trait ClonableCompile: Compile + Cloner {}
impl<T> ClonableCompile for T where T: Compile + Clone + 'static {}

impl Clone for for<'a> fn(&mut Item) {
    fn clone(&self) -> Self {
        *self
    }
}

impl Clone for Box<ClonableCompile + Send + Sync> {
    fn clone(&self) -> Box<ClonableCompile + Send + Sync> {
        (**self).clone_compiler()
    }
}

impl<C> Compile for Arc<C> where C: Compile {
    fn compile(&self, item: &mut Item) {
        (**self).compile(item)
    }
}

impl<C: ?Sized> Compile for Box<C> where C: Compile {
    fn compile(&self, item: &mut Item) {
        (**self).compile(item)
    }
}

// impl<C: ?Sized> Compile for &'static C where C: Compile {
//     fn compile(&self, item: &mut Item) {
//         (**self).compile(item)
//     }
// }

// impl<C: ?Sized> Compile for &'static mut C where C: Compile {
//     fn compile(&self, item: &mut Item) {
//         (**self).compile(item);
//     }
// }

impl<F> Compile for F where F: Fn(&mut Item) + Send + Sync {
    fn compile(&self, item: &mut Item) {
        self(item)
    }
}

// FIXME: this completely ignores barriers
// impl Compile for Arc<Vec<Link>> {
//     fn compile(&self, item: &mut Item) {
//         for link in self.iter() {
//             if let &Link::Compiler(ref compiler) = link {
//                 (*compiler).compile(item);
//             }
//         }
//     }
// }

#[derive(Clone)]
pub enum Link {
    Normal(Box<ClonableCompile + Send + Sync>),
    Barrier,
}

#[derive(Copy, Debug)]
pub enum Status {
    Continue,
    Pause,
}

/// Chain of compilers.
///
/// Maintains a list of compilers and executes them
/// in the order they were added.
pub struct Chain {
    pub chain: Vec<Link>,
}

impl Clone for Chain {
    fn clone(&self) -> Chain {
        Chain {
            chain: self.chain.clone(),
        }
    }
}

#[derive(Clone)]
pub struct ChainPosition { trail: Vec<usize> }

impl Chain {
    pub fn new() -> Chain {
        Chain {
            chain: vec![],
        }
    }

    pub fn only<C>(compiler: C) -> Chain
    where C: Compile + Clone + 'static {
        Chain {
            chain: vec![Link::Normal(Box::new(compiler) as Box<ClonableCompile + Send + Sync>)],
        }
    }

    pub fn link<C>(mut self, compiler: C) -> Chain
    where C: Compile + Clone + 'static {
        self.chain.push(Link::Normal(Box::new(compiler) as Box<ClonableCompile + Send + Sync>));
        self
    }

    pub fn barrier(mut self) -> Chain {
        self.chain.push(Link::Barrier);
        self
    }
}

pub fn is_paused(item: &Item) -> bool {
    item.data.get::<ChainPosition>()
        .map(|c| !c.trail.is_empty())
        .unwrap_or(false)
}

fn save_chain_position(item: &mut Item, index: usize) {
    item.data.entry::<ChainPosition>().get()
        .unwrap_or_else(|v| v.insert(ChainPosition { trail: Vec::new() }))
        .trail.push(index);
}

fn get_chain_position(item: &mut Item) -> usize {
    item.data.get_mut::<ChainPosition>()
        .and_then(|chain| chain.trail.pop())
        .unwrap_or(0)
}

impl Compile for Chain {
    fn compile(&self, item: &mut Item) {
        let resuming = is_paused(item);
        let position: usize = get_chain_position(item);

        for (index, link) in (position ..).zip(self.chain[position ..].iter()) {
            match *link {
                Link::Barrier => {
                    // if we are resuming and we begin on a barrier, skip over it,
                    // since we just performed the barrier
                    if resuming && index == position {
                        continue;
                    }

                    save_chain_position(item, index);
                    return;
                },
                Link::Normal(ref compiler) => {
                    compiler.compile(item);

                    if is_paused(item) {
                        save_chain_position(item, index);
                        return;
                    }
                },
            }
        }

        return;
    }
}

pub fn stub(item: &mut Item) {
    trace!("no compiler established for: {:?}", item);
}

/// Compiler that reads the `Item`'s body.
pub fn read(item: &mut Item) {
    use std::fs::File;
    use std::io::Read;

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
            let target = conf_out.join(path);

            if !target.starts_with(&conf_out) {
                panic!("attempted to write outside of the output directory: {:?}", target);
            }

            trace!("mkdir -p {:?}", target);

            if let Some(parent) = target.parent() {
                // TODO: this errors out if the path already exists? dumb
                let _ = fs::create_dir_all(parent);
            }

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

pub fn inject_with<T>(t: Arc<T>) -> Arc<Box<Compile + Sync + Send>>
where T: Sync + Send + 'static {
    Arc::new(Box::new(move |item: &mut Item| {
        item.data.insert(t.clone());
    }))
}

// TODO: this isn't necessary. in fact, on Barriers, it ends up cloning the data
// this is mainly to ensure that the user is passing an Arc
#[derive(Clone)]
pub struct Inject<T> where T: Sync + Send + Clone + 'static {
    data: T,
}

impl<T> Inject<T> where T: Sync + Send + Clone + 'static {
    pub fn with(t: T) -> Inject<T>
    where T: Sync + Send + Clone + 'static {
        Inject {
            data: t
        }
    }
}

impl<T> Compile for Inject<T> where T: Sync + Send + Clone + 'static {
    fn compile(&self, item: &mut Item) {
        item.data.insert(self.data.clone());
    }
}

use rustc_serialize::json::Json;
use handlebars::Handlebars;

pub fn render_template<H>(name: &'static str, handler: H) -> Arc<Box<Compile + Sync + Send>>
where H: Fn(&Item) -> Json + Sync + Send + 'static {
    Arc::new(Box::new(move |item: &mut Item| {
        if let Some(ref registry) = item.data.get::<Arc<Handlebars>>() {
            let json = handler(item);
            item.body = Some(registry.render(name, &json).unwrap());
        }
    }))
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

pub fn only_if<C, F>(condition: C, mut compiler: F) -> Arc<Box<Compile + Sync + Send>>
where C: Fn(&Item) -> bool + Sync + Send + 'static,
      F: Compile + Sync + Send + 'static {
    Arc::new(Box::new(move |item: &mut Item| {
        if condition(item) {
            return compiler.compile(item);
        }
    }))
}

// struct OnlyIf<B, C> where B: Fn(&Item) -> bool + Sync + Send + 'static, C: Compile + Sync + Send + 'static {
//     condition:  B,
//     compiler: C,
// }

// impl Compile for RenderTemplate {
//     fn compile(&self, item: &mut Item) {
//     }
// }

