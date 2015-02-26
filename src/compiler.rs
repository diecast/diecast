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
    fn compile(&self, item: &mut Item) -> Status;
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

impl Clone for for<'a> fn(&mut Item) -> Status {
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
    fn compile(&self, item: &mut Item) -> Status {
        (**self).compile(item)
    }
}

impl<C: ?Sized> Compile for Box<C> where C: Compile {
    fn compile(&self, item: &mut Item) -> Status {
        (**self).compile(item)
    }
}

// impl<C: ?Sized> Compile for &'static C where C: Compile {
//     fn compile(&self, item: &mut Item) -> Status {
//         (**self).compile(item)
//     }
// }

// impl<C: ?Sized> Compile for &'static mut C where C: Compile {
//     fn compile(&self, item: &mut Item) {
//         (**self).compile(item);
//     }
// }

impl<F> Compile for F where F: Fn(&mut Item) -> Status + Send + Sync {
    fn compile(&self, item: &mut Item) -> Status {
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
pub struct Compiler {
    pub chain: Vec<Link>,
}

impl Clone for Compiler {
    fn clone(&self) -> Compiler {
        Compiler {
            chain: self.chain.clone(),
        }
    }
}

#[derive(Clone)]
pub struct ChainPosition { trail: Vec<usize> }

impl Compiler {
    pub fn new() -> Compiler {
        Compiler {
            chain: vec![],
        }
    }

    pub fn only<C>(compiler: C) -> Compiler
    where C: Compile + Clone + 'static {
        Compiler {
            chain: vec![Link::Normal(Box::new(compiler) as Box<ClonableCompile + Send + Sync>)],
        }
    }

    pub fn link<C>(mut self, compiler: C) -> Compiler
    where C: Compile + Clone + 'static {
        self.chain.push(Link::Normal(Box::new(compiler) as Box<ClonableCompile + Send + Sync>));
        self
    }

    pub fn barrier(mut self) -> Compiler {
        self.chain.push(Link::Barrier);
        self
    }
}

impl Compile for Compiler {
    fn compile(&self, item: &mut Item) -> Status {
        let itm = {
            use std::fmt::Write;
            let mut buf = String::new();
            let _ = buf.write_fmt(format_args!("{:?}", item));
            buf.shrink_to_fit();
            buf
        };

        // get position from item.data
        let position: usize =
            item.data.get_mut::<ChainPosition>()
                .and_then(|chain| { chain.trail.pop().map(|p| p + 1) })
                .unwrap_or(0);

        println!("{:?} -- restored position: {}", itm, position);

        for (index, link) in (position ..).zip(self.chain[position ..].iter()) {
            println!("{:?} -- position: {}, index: {}", itm, position, index);

            match *link {
                Link::Barrier => {
                    println!("{:?} barrier encountered", itm);
                    let chain_pos =
                        item.data.entry::<ChainPosition>().get()
                        .unwrap_or_else(|v| v.insert(ChainPosition { trail: Vec::new() }));

                    println!("{:?} before push: {:?}", itm, chain_pos.trail);

                    chain_pos.trail.push(index);

                    println!("{:?} after push: {:?}", itm, chain_pos.trail);

                    return Status::Pause;
                },
                Link::Normal(ref compiler) => {
                    match compiler.compile(item) {
                        Status::Pause => {
                            println!("compiler paused");
                            let chain_pos =
                                item.data.entry::<ChainPosition>().get()
                                .unwrap_or_else(|v| v.insert(ChainPosition { trail: Vec::new() }));

                            chain_pos.trail.push(index);
                            return Status::Pause;
                        },
                        Status::Continue => {
                            println!("{:?} -- DONE!", itm);
                        },
                    }
                },
            }
        }

        return Status::Continue;
    }
}

pub fn stub(item: &mut Item) -> Status {
    trace!("no compiler established for: {:?}", item);
    Status::Continue
}

/// Compiler that reads the `Item`'s body.
pub fn read(item: &mut Item) -> Status {
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

    Status::Continue
}

/// Compiler that writes the `Item`'s body.
pub fn write(item: &mut Item) -> Status {
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

    Status::Continue
}


/// Compiler that prints the `Item`'s body.
pub fn print(item: &mut Item) -> Status {
    use std::old_io::stdio::println;

    if let &Some(ref body) = &item.body {
        println(body);
    }

    Status::Continue
}

#[derive(Clone)]
pub struct Metadata(pub String);

pub fn parse_metadata(item: &mut Item) -> Status {
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
                return Status::Continue;
            } else {
                item.body = None;
                return Status::Continue;
            }
        }

        item.body = Some(body);
    }

    Status::Continue
}

#[derive(Clone)]
pub struct TomlMetadata(pub toml::Value);

pub fn parse_toml(item: &mut Item) -> Status {
    let parsed = if let Some(&Metadata(ref parsed)) = item.data.get::<Metadata>() {
        Some(parsed.parse().unwrap())
    } else {
        None
    };

    if let Some(parsed) = parsed {
        item.data.insert(TomlMetadata(parsed));
    }

    Status::Continue
}

pub fn render_markdown(item: &mut Item) -> Status {
    use hoedown::Markdown;
    use hoedown::renderer::html;

    if let Some(body) = item.body.take() {
        let document = Markdown::new(body.as_bytes());
        let renderer = html::Html::new(html::Flags::empty(), 0);
        let buffer = document.render_to_buffer(renderer);
        item.data.insert(buffer);
        item.body = Some(body);
    }

    Status::Continue
}

pub fn inject_with<T>(t: Arc<T>) -> Arc<Box<Compile + Sync + Send>>
where T: Sync + Send + 'static {
    Arc::new(Box::new(move |item: &mut Item| -> Status {
        item.data.insert(t.clone());
        Status::Continue
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
    fn compile(&self, item: &mut Item) -> Status {
        item.data.insert(self.data.clone());
        Status::Continue
    }
}

use rustc_serialize::json::Json;
use handlebars::Handlebars;

pub fn render_template<H>(name: &'static str, handler: H) -> Arc<Box<Compile + Sync + Send>>
where H: Fn(&Item) -> Json + Sync + Send + 'static {
    Arc::new(Box::new(move |item: &mut Item| -> Status {
        if let Some(ref registry) = item.data.get::<Arc<Handlebars>>() {
            let json = handler(item);
            item.body = Some(registry.render(name, &json).unwrap());
        }

        Status::Continue
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
    fn compile(&self, item: &mut Item) -> Status {
        if let Some(ref registry) = item.data.get::<Arc<Handlebars>>() {
            let json = (*self.handler)(item);
            item.body = Some(registry.render(self.name, &json).unwrap());
        }

        Status::Continue
    }
}

pub fn only_if<C, F>(condition: C, mut compiler: F) -> Arc<Box<Compile + Sync + Send>>
where C: Fn(&Item) -> bool + Sync + Send + 'static,
      F: Compile + Sync + Send + 'static {
    Arc::new(Box::new(move |item: &mut Item| -> Status {
        if condition(item) {
            return compiler.compile(item);
        }

        Status::Continue
    }))
}

// struct OnlyIf<B, C> where B: Fn(&Item) -> bool + Sync + Send + 'static, C: Compile + Sync + Send + 'static {
//     condition:  B,
//     compiler: C,
// }

// impl Compile for RenderTemplate {
//     fn compile(&self, item: &mut Item) -> Status {
//     }
// }

