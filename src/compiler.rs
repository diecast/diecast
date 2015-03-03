//! Compiler behavior.

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};

use toml;

use item::Item;

pub type Result = ::std::result::Result<(), Box<::std::error::Error>>;

/// Behavior of a compiler.
///
/// There's a single method that takes a mutable
/// reference to the `Item` being compiled.
pub trait Compile: Send + Sync {
    fn compile(&self, item: &mut Item) -> Result;
}

impl<C> Compile for Arc<C> where C: Compile {
    fn compile(&self, item: &mut Item) -> Result {
        (**self).compile(item)
    }
}

impl<C: ?Sized> Compile for Box<C> where C: Compile {
    fn compile(&self, item: &mut Item) -> Result {
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

impl<F> Compile for F where F: Fn(&mut Item) -> Result + Send + Sync {
    fn compile(&self, item: &mut Item) -> Result {
        self(item)
    }
}

#[derive(Clone)]
pub enum Link {
    Normal(Arc<Box<Compile + Send + Sync>>),
    Barrier,
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
    where C: Compile + 'static {
        Chain {
            chain: vec![Link::Normal(Arc::new(Box::new(compiler) as Box<Compile + Send + Sync>))],
        }
    }

    pub fn link<C>(mut self, compiler: C) -> Chain
    where C: Compile + 'static {
        self.chain.push(Link::Normal(Arc::new(Box::new(compiler) as Box<Compile + Send + Sync>)));
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
    fn compile(&self, item: &mut Item) -> Result {
        let resuming = is_paused(item);
        let position: usize = get_chain_position(item);

        // item.data.entry::<ChainBarriers>().get()
        //     .unwrap_or_else(|v| v.insert(ChainBarriers { barriers: Vec::new() }));

        for (index, link) in (position ..).zip(self.chain[position ..].iter()) {
            match *link {
                Link::Barrier => {
                    // if we are resuming and we begin on a barrier, skip over it,
                    // since we just performed the barrier
                    if resuming && index == position {
                        continue;
                    }

                    save_chain_position(item, index);
                    return Ok(());
                },
                Link::Normal(ref compiler) => {
                    try!(compiler.compile(item));

                    if is_paused(item) {
                        save_chain_position(item, index);

                        // here check if it's the last item in the barriers?

                        return Ok(());
                    }
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

/// Compiler that reads the `Item`'s body.
pub fn read(item: &mut Item) -> Result {
    use std::fs::File;
    use std::io::Read;

    if let Some(ref path) = item.from {
        let mut buf = String::new();

        // TODO: use try!
        File::open(&item.configuration.input.join(path))
            .unwrap()
            .read_to_string(&mut buf)
            .unwrap();

        item.body = Some(buf);
    }

    Ok(())
}

/// Compiler that writes the `Item`'s body.
pub fn write(item: &mut Item) -> Result {
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

    Ok(())
}


/// Compiler that prints the `Item`'s body.
pub fn print(item: &mut Item) -> Result {
    use std::old_io::stdio::println;

    if let &Some(ref body) = &item.body {
        println(body);
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

pub fn inject_with<T>(t: Arc<T>) -> Box<Compile + Sync + Send>
where T: Sync + Send + 'static {
    Box::new(move |item: &mut Item| -> Result {
        item.data.insert(t.clone());
        Ok(())
    })
}

// TODO: this isn't necessary. in fact, on Barriers, it ends up cloning the data
// this is mainly to ensure that the user is passing an Arc
// #[derive(Clone)]
// pub struct Inject<T> where T: Sync + Send + Clone + 'static {
//     data: T,
// }

// impl<T> Inject<T> where T: Sync + Send + Clone + 'static {
//     pub fn with(t: T) -> Inject<T>
//     where T: Sync + Send + Clone + 'static {
//         Inject {
//             data: t
//         }
//     }
// }

// impl<T> Compile for Inject<T> where T: Sync + Send + Clone + 'static {
//     fn compile(&self, item: &mut Item) {
//         item.data.insert(self.data.clone());
//     }
// }

use rustc_serialize::json::Json;
use handlebars::Handlebars;

pub fn render_template<H>(name: &'static str, handler: H) -> Box<Compile + Sync + Send>
where H: Fn(&Item) -> Json + Sync + Send + 'static {
    Box::new(move |item: &mut Item| -> Result {
        if let Some(ref registry) = item.data.get::<Arc<Handlebars>>() {
            let json = handler(item);
            item.body = Some(registry.render(name, &json).unwrap());
        }

        Ok(())
    })
}

// pub struct RenderTemplate {
//     name: &'static str,
//     handler: Box<Fn(&Item) -> Json + Sync + Send + 'static>,
// }

// impl RenderTemplate {
//     pub fn new<H>(name: &'static str, handler: H) -> RenderTemplate
//     where H: Fn(&Item) -> Json + Sync + Send + 'static {
//         RenderTemplate {
//             name: name,
//             handler: Box::new(handler),
//         }
//     }
// }

// impl Compile for RenderTemplate {
//     fn compile(&self, item: &mut Item) {
//         if let Some(ref registry) = item.data.get::<Arc<Handlebars>>() {
//             let json = (*self.handler)(item);
//             item.body = Some(registry.render(self.name, &json).unwrap());
//         }
//     }
// }

#[derive(Clone)]
pub struct Barriers {
    // TODO
    // this needs to be in a mutex because there needs to be only one of these
    pub counts: Arc<Mutex<Vec<usize>>>,
}

pub struct OnlyIf<B, C>
where B: Fn(&Item) -> bool + Sync + Send + 'static,
      C: Compile + Sync + Send + 'static {
    // only run `compiler` if this condition holds true
    condition:  Arc<B>,
    // compiler to conditionally run
    compiler: Arc<C>,
    // count of items that satisfy predicate,
    // so that subsequent barriers nested within the above `compiler`
    // only block on this amount of items
    ready: Arc<AtomicUsize>,
    // this ensures that the stack is only pushed/popped once per OnlyIf
    is_pushed: Arc<Mutex<bool>>,
}

impl<B, C> OnlyIf<B, C>
where B: Fn(&Item) -> bool + Sync + Send + 'static,
      C: Compile + Sync + Send + 'static {
    fn new(condition: B, compiler: C) -> OnlyIf<B, C> {
        OnlyIf {
            condition: Arc::new(condition),
            compiler: Arc::new(compiler),
            ready: Arc::new(AtomicUsize::new(0)),
            is_pushed: Arc::new(Mutex::new(false)),
        }
    }
}

impl<B, C> Compile for OnlyIf<B, C>
where B: Fn(&Item) -> bool + Sync + Send + 'static,
      C: Compile + Sync + Send + 'static {
    fn compile(&self, item: &mut Item) -> Result {
        let satisfied = (self.condition)(item);

        // this kinda sucks but w/e
        let ready_1 = self.ready.clone();
        let ready_2 = self.ready.clone();

        let is_pushed_1 = self.is_pushed.clone();
        let is_pushed_2 = self.is_pushed.clone();

        let compiler = self.compiler.clone();

        // eww, guess I'll at least make this a data member to avoid
        // rebuilding it each time
        //
        // this needs to be an arc afaik cause in `run_it` it's passed
        // to a chain, and if it weren't an arc then it'd move it
        // which would make `run_it` an FnOnce instead of the Fn we need
        let pop_it =
            Arc::new(move |item: &mut Item| -> Result {
                let is_pushed_2 = is_pushed_2.clone();
                let mut is_pushed_2 = is_pushed_2.lock().unwrap();

                // will pop the barrier count at the end of the conditional compiler
                // if it hasn't been popped yet
                if *is_pushed_2 {
                    let barriers = item.data.get_mut::<Barriers>().unwrap();
                    let mut counts = barriers.counts.lock().unwrap();
                    counts.pop();

                    *is_pushed_2 = false;
                }

                Ok(())
            });

        // this is also nasty to have to build this here, but w/e
        let run_it =
            move |item: &mut Item| -> Result {
                if satisfied {
                    // perhaps make this a member to avoid rebuilding it each time?
                    return Chain::new()
                        .link(compiler.clone())
                        .barrier()
                        .link(pop_it.clone())
                        .compile(item);
                }

                Ok(())
            };

        Chain::new()
            // count items that satisfy the predicate
            .link(move |item: &mut Item| -> Result {
                if !satisfied {
                    return Ok(())
                }

                let new_count = ready_1.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
            .barrier()
            // push count
            .link(move |item: &mut Item| -> Result {
                if !satisfied {
                    return Ok(())
                }

                let is_pushed_1 = is_pushed_1.clone();
                let mut is_pushed_1 = is_pushed_1.lock().unwrap();

                // if the new barrier count hasn't been pushed, push it
                if !*is_pushed_1 {
                    let max_count = ready_2.load(Ordering::SeqCst);

                    let barriers = item.data.entry::<Barriers>().get()
                        .unwrap_or_else(|v| {
                            v.insert(Barriers { counts: Arc::new(Mutex::new(Vec::new())) })
                        });

                    let mut counts = barriers.counts.lock().unwrap();
                    counts.push(max_count);

                    *is_pushed_1 = true;
                }

                Ok(())
            })
            // run compiler then pop barrier stack
            .link(run_it)
            // run this compiler chain
            .compile(item)
    }
}

// helper function. `OnlyIf` itself is not exposed cause who wants to type that
pub fn only_if<C, F>(condition: C, compiler: F) -> OnlyIf<C, F>
where C: Fn(&Item) -> bool + Sync + Send + 'static,
      F: Compile + Sync + Send + 'static {
    OnlyIf::new(condition, compiler)
}

