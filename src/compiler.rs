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
    condition:  Arc<B>,
    compiler: Arc<C>,
    ready: Arc<AtomicUsize>,
    is_pushed: Arc<Mutex<bool>>,
    is_popped: Arc<Mutex<bool>>,
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
            is_popped: Arc::new(Mutex::new(false)),
        }
    }
}

impl<B, C> Compile for OnlyIf<B, C>
where B: Fn(&Item) -> bool + Sync + Send + 'static,
      C: Compile + Sync + Send + 'static {
    fn compile(&self, item: &mut Item) -> Result {
        let satisfied = (*self.condition)(item);

        let ready_1 = self.ready.clone();
        let ready_2 = self.ready.clone();

        let is_pushed = self.is_pushed.clone();
        let is_popped = self.is_popped.clone();

        let compiler = self.compiler.clone();
        let pop_it =
            Arc::new(move |item: &mut Item| -> Result {
                let is_popped = is_popped.clone();
                let mut is_popped = is_popped.lock().unwrap();

                if !*is_popped {
                    // unwrap should be fine here since we ensured it exists
                    // FIXME: although, I guess it could be possible that
                    // an intermediate compiler removed it
                    // probably a losing battle to attempt to handle all
                    // of that, so better to just panic imo

                    println!("{} -- popped count", item);

                    let barriers = item.data.get_mut::<Barriers>().unwrap();
                    let mut counts = barriers.counts.lock().unwrap();
                    counts.pop();

                    *is_popped = true;
                }

                Ok(())
            });

        let run_it =
            move |item: &mut Item| -> Result {
                if satisfied {
                    println!("{} -- running compiler because pred satisfied", item);
                    return Chain::new()
                        .link(compiler.clone())
                        .barrier()
                        .link(pop_it.clone())
                        .compile(item);
                }

                println!("{} -- didn't run compiler cause !pred", item);
                Ok(())
            };

        Chain::new()
            .link(move |item: &mut Item| -> Result {
                if !satisfied {
                    println!("{} -- did not satisfy the predicate", item);
                    return Ok(())
                }

                // TODO
                // need to ensure that this is actually a clone of the original
                // and not a completely new one each time
                let new_count = ready_1.fetch_add(1, Ordering::SeqCst);
                println!("#{}: {} -- has reached the barrier", new_count, item);
                Ok(())
            })
            // allow all passed items to inc count
            // FIXME
            // this barrier wont work for the same reason we're
            // trying to fix it!
            //
            // now all satisfied items will have incremented it
            .barrier()
            // push count
            .link(move |item: &mut Item| -> Result {
                if !satisfied {
                    println!("{} -- not pushing count cause !pred", item);
                    return Ok(())
                }

                println!("{} -- pushing count", item);

                // TODO: audit ordering
                // TODO: audit use of comparison to 0
                // what if no items matched?
                // well in that case we wouldn't even be running this
                // code, because `satisfied` sould be false
                let max_count = ready_2.load(Ordering::SeqCst);
                let is_pushed = is_pushed.clone();

                let mut is_pushed = is_pushed.lock().unwrap();

                // NOTE:
                // it isn't necessary to wrap the Barriers count in a mutex I think
                // since, sure, we'd have atomic access to the vec but we wouldn't know
                // if it had been set yet or not
                if !*is_pushed {
                    println!("{} -- pushed count", item);

                    let barriers = item.data.entry::<Barriers>().get()
                        .unwrap_or_else(|v| v.insert(Barriers { counts: Arc::new(Mutex::new(Vec::new())) }));

                    let mut counts = barriers.counts.lock().unwrap();
                    counts.push(max_count);

                    *is_pushed = true;
                }

                Ok(())
            })
            .link(run_it)
            // done compiling, reset barrier count
            // should only happen once all have finished
            // another barrier needed?
            //
            // NOTE
            // by the time this barrier runs, the new update count should be set
            // so there's no need for dual atomics?
            //
            // FIXME
            // this barrier still uses the new count, but will still
            // be triggered by items that didn't match the predicate
            .compile(item)
    }
}

pub fn only_if<C, F>(condition: C, compiler: F) -> OnlyIf<C, F>
where C: Fn(&Item) -> bool + Sync + Send + 'static,
      F: Compile + Sync + Send + 'static {
    OnlyIf::new(condition, compiler)
}

