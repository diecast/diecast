use std::sync::Arc;
use std::collections::HashMap;
use std::any::Any;
use std::path::PathBuf;

use typemap;

use job::evaluator::Pool;
use item::{Item, Route};
use bind::Bind;
use handle::{self, Handle, Result};

use super::{Chain, Extender};

pub struct Create {
    path: PathBuf,
}

impl Handle<Bind> for Create {
    fn handle(&self, bind: &mut Bind) -> handle::Result {
        let data = bind.get_data();
        bind.items_mut()
            .push(Item::new(Route::Write(self.path.clone()), data));
        Ok(())
    }
}

#[inline]
pub fn create<P>(path: P) -> Create
where P: Into<PathBuf> {
    Create {
        path: path.into(),
    }
}

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

pub struct Retain<C>
where C: Fn(&Item) -> bool, C: Sync + Send + 'static {
    condition: C,
}

impl<C> Handle<Bind> for Retain<C>
where C: Fn(&Item) -> bool, C: Sync + Send + 'static {
    fn handle(&self, bind: &mut Bind) -> handle::Result {
        bind.items_mut().retain(&self.condition);
        Ok(())
    }
}

#[inline]
pub fn retain<C>(condition: C) -> Retain<C>
where C: Fn(&Item) -> bool, C: Copy + Sync + Send + 'static {
    Retain {
        condition: condition,
    }
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
    fn handle(&self, bind: &mut Bind) -> Result {
        for item in bind.iter_mut() {
            try!(self.handler.handle(item));
        }

        Ok(())
    }
}

impl Handle<Bind> for Chain<Bind> {
    fn handle(&self, bind: &mut Bind) -> Result {
        for handler in &self.handlers {
            try!(handler.handle(bind));
        }

        Ok(())
    }
}

impl<T> Handle<Bind> for Extender<T>
where T: typemap::Key, T::Value: Any + Sync + Send + Clone {
    fn handle(&self, bind: &mut Bind) -> handle::Result {
        bind.data().extensions.write().unwrap().insert::<T>(self.payload.clone());
        Ok(())
    }
}

// TODO
// should this probably be a separate crate?
// store mutex<threadpool> in extensions,
// then this handler would use it?
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
        let total = bind.items().len();

        let mut items = ::std::mem::replace(bind.items_mut(), vec![]);
        let mut retainer = vec![];

        // if it's updating, then we should collect the
        if bind.is_stale() {
            let (stale, ignore): (Vec<_>, Vec<_>) =
                items.into_iter().partition(|i| i.is_stale());

            items = stale;
            retainer = ignore;
        }

        let item_count = items.len();
        let chunks = {
            let (div, rem) = (item_count / self.chunk, item_count % self.chunk);

            if rem == 0 {
                div
            } else {
                div + 1
            }
        };

        // TODO: optimize this for general case of chunk=1?
        while !items.is_empty() {
            let rest = if self.chunk > items.len() {
                vec![]
            } else {
                let xs = items;
                items = vec![];
                let mut rest = vec![];

                // TODO
                // less efficient than split_off which is unstable
                for (i, itm) in xs.into_iter().enumerate() {
                    if i < self.chunk {
                        items.push(itm);
                    } else {
                        rest.push(itm);
                    }
                }

                rest
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
            bind.items_mut().extend(pool.dequeue().unwrap().into_iter());
        }

        if !retainer.is_empty() {
            bind.items_mut().extend(retainer.into_iter());
        }

        assert_eq!(total, bind.items().len());

        Ok(())
    }
}

pub fn missing(_bind: &mut Bind) -> handle::Result {
    trace!("missing handler");
    Ok(())
}

#[derive(Clone, Debug)]
pub struct Adjacent {
    previous: Option<Arc<Item>>,
    next: Option<Arc<Item>>,
}

impl typemap::Key for Adjacent {
    type Value = Adjacent;
}

pub fn adjacent(bind: &mut Bind) -> handle::Result {
    let count = bind.items().len();

    let last_num = if count == 0 {
        0
    } else {
        count - 1
    };

    // TODO: yet another reason to have Arc<Item>?
    // FIXME
    // the problem with this is that unlike Paginate,
    // it'll contain copies of the item Should probably
    // instead insert an index?
    let cloned =
        bind.items().iter()
        .map(|i| Arc::new(i.clone()))
        .collect::<Vec<Arc<Item>>>();

    for (idx, item) in bind.items_mut().iter_mut().enumerate() {
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

pub struct Tags;

impl typemap::Key for Tags {
    type Value = HashMap<String, Arc<Vec<Arc<Item>>>>;
}

pub fn tags(bind: &mut Bind) -> handle::Result {
    let mut tag_map = ::std::collections::HashMap::new();

    for item in bind.iter() {
        let toml =
            item.extensions.get::<super::item::Metadata>()
            .and_then(|m| m.lookup("tags"))
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

    bind.data().extensions.write().unwrap().insert::<Tags>(arc_map);

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
        bind.items_mut().sort_by(|a, b| -> ::std::cmp::Ordering {
            (self.compare)(a, b)
        });

        Ok(())
    }
}

