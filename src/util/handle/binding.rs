use std::sync::Arc;
use std::path::{PathBuf, Path};
use std::collections::HashMap;
use std::any::Any;
use std::fs;

use chrono;

use job::evaluator::Pool;
use item::{Item, Route};
use binding::Bind;
use handle::{self, Handle, Result};
use pattern::Pattern;

use super::{Chain, Extender};
use super::item;

pub fn each<H>(handler: H) -> Each<H>
where H: Handle<Item> {
    Each {
        handler: handler,
    }
}

pub struct Each<H>
where H: Handle<Item> {
    handler: H,
}

// pub fn static_file<P>(pattern: P) -> Chain<Bind>
// where P: Pattern + Sync + Send + 'static {
//     Chain::new()
//     .link(select(pattern))
//     .link(each(Chain::new()
//         .link(::util::route::identity)
//         .link(item::copy)))
// }

impl<H> Handle<Bind> for Each<H>
where H: Handle<Item> {
    fn handle(&self, binding: &mut Bind) -> Result {
        for item in binding.iter_mut() {
            try!(self.handler.handle(item));
        }

        Ok(())
    }
}

impl Handle<Bind> for Chain<Bind> {
    fn handle(&self, binding: &mut Bind) -> Result {
        for handler in &self.handlers {
            try!(handler.handle(binding));
        }

        Ok(())
    }
}

impl<T> Handle<Bind> for Extender<T>
where T: Any + Sync + Send + Clone + 'static {
    fn handle(&self, bind: &mut Bind) -> handle::Result {
        bind.data().extensions.write().unwrap().insert(self.payload.clone());
        Ok(())
    }
}

pub fn parallel_each<H>(handler: H) -> ParallelEach<H>
where H: Handle<Item> + Sync + Send + 'static {
    ParallelEach {
        chunk: 1,
        handler: Arc::new(handler),
    }
}

// TODO: should the chunk be in configuration or a parameter?
pub struct ParallelEach<H>
where H: Handle<Item> + Sync + Send + 'static {
    chunk: usize,
    handler: Arc<H>,
}

impl<H> ParallelEach<H>
where H: Handle<Item> + Sync + Send + 'static {
    pub fn chunk(mut self, size: usize) -> ParallelEach<H> {
        self.chunk = size;
        self
    }
}

impl<H> Handle<Bind> for ParallelEach<H>
where H: Handle<Item> + Sync + Send + 'static {
    fn handle(&self, bind: &mut Bind) -> handle::Result {
        let pool: Pool<Vec<Item>> = Pool::new(bind.data().configuration.threads);
        let item_count = bind.len();

        let chunks = {
            let (div, rem) = (item_count / self.chunk, item_count % self.chunk);

            if rem == 0 {
                div
            } else {
                div + 1
            }
        };

        // FIXME: drain() won't be stable!
        let mut items = unsafe { ::std::mem::replace(bind.items_mut(), vec![]) };

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
                    match <Handle<Item>>::handle(&handler, &mut item) {
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
            // TODO: this completely defeats the purpose of hiding the items field
            unsafe { bind.items_mut().extend(pool.dequeue().unwrap().into_iter()) };
        }

        assert!(item_count == bind.len(), "received different number of items from pool");

        Ok(())
    }
}

pub fn stub(_bind: &mut Bind) -> handle::Result {
    trace!("stub handler");
    Ok(())
}

#[derive(Clone, Debug)]
pub struct Adjacent {
    previous: Option<Arc<Item>>,
    next: Option<Arc<Item>>,
}

pub fn next_prev(bind: &mut Bind) -> handle::Result {
    let count = bind.len();

    let last_num = if count == 0 {
        0
    } else {
        count - 1
    };

    // TODO: yet another reason to have Arc<Item>?
    let cloned = bind.iter().map(|i| Arc::new(i.clone())).collect::<Vec<Arc<Item>>>();

    for (idx, item) in bind.iter_mut().enumerate() {
        let prev =
            if idx == 0 { None }
            else { let num = idx - 1; Some(cloned[num].clone()) };
        let next =
            if idx == last_num { None }
            else { let num = idx + 1; Some(cloned[num].clone()) };

        item.extensions.insert::<Adjacent>(Adjacent {
            previous: prev,
            next: next,
        });
    }

    Ok(())
}

#[derive(Clone)]
pub struct Tags {
    pub map: HashMap<String, Arc<Vec<Arc<Item>>>>,
}

pub fn tags(bind: &mut Bind) -> handle::Result {
    let mut tag_map = ::std::collections::HashMap::new();

    for item in bind.iter() {
        let toml =
            item.extensions.get::<super::item::Metadata>()
            .and_then(|m| {
                m.data.lookup("tags")
            })
            .and_then(::toml::Value::as_slice);

        let arc = Arc::new(item.clone());

        if let Some(tags) = toml {
            for tag in tags {
                tag_map.entry(String::from(tag.as_str().unwrap()))
                    .or_insert(vec![])
                    .push(arc.clone());
            }
        }
    }

    let mut arc_map = HashMap::new();

    for (k, v) in tag_map {
        arc_map.insert(k, Arc::new(v));
    }

    bind.data().extensions.write().unwrap().insert::<Tags>(Tags { map: arc_map });

    Ok(())
}

pub struct SortBy<F>
where F: Fn(&Item, &Item) -> ::std::cmp::Ordering,
      F: Sync + Send + 'static {
    compare: F,
}

pub fn sort_by<F>(compare: F) -> SortBy<F>
where F: Fn(&Item, &Item) -> ::std::cmp::Ordering,
      F: Sync + Send + 'static {
    SortBy {
        compare: compare,
    }
}

impl<F> Handle<Bind> for SortBy<F>
where F: Fn(&Item, &Item) -> ::std::cmp::Ordering,
      F: Sync + Send + 'static {
    fn handle(&self, bind: &mut Bind) -> handle::Result {
        unsafe {
            bind.items_mut().sort_by(|a, b| -> ::std::cmp::Ordering {
                (self.compare)(a, b)
            });
        }

        Ok(())
    }
}

