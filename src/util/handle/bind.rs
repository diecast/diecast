use std::sync::Arc;
use std::any::Any;
use std::path::PathBuf;
use std::{cmp, mem};

use typemap;

use futures::prelude::*;
use futures::{self, future, Future};

use item::Item;
use bind::Bind;
use handler::Handle;
use pattern::Pattern;

use super::Extender;

pub struct InputPaths;

impl typemap::Key for InputPaths {
    type Value = Arc<Vec<PathBuf>>;
}

impl<T> Handle<Bind> for Extender<T>
where T: typemap::Key, T::Value: Any + Sync + Send + Clone {
    fn handle(&self, bind: &mut Bind) -> ::Result<()> {
        bind.extensions.write().unwrap().insert::<T>(self.payload.clone());
        Ok(())
    }
}

pub struct Create {
    path: PathBuf,
}

impl Handle<Bind> for Create {
    fn handle(&self, bind: &mut Bind) -> ::Result<()> {
        bind.attach(Item::writing(self.path.clone()));

        Ok(())
    }
}

pub struct Select<P>
where P: Pattern + Sync + Send + 'static {
    pattern: P,
}

impl<P> Handle<Bind> for Select<P>
where P: Pattern + Sync + Send + 'static {
    fn handle(&self, bind: &mut Bind) -> ::Result<()> {
        let paths = bind.extensions.read().unwrap().get::<InputPaths>().unwrap().clone();

        for path in paths.iter() {
            let relative = try!(path.strip_prefix(&bind.configuration.input)).to_path_buf();

            // TODO
            // decide how to handle pattern matching consistently
            // for example, Configuration::ignore matches on the file_name,
            // but this pattern seems to be matching on the whole pattern rooted
            // at the input directory
            if self.pattern.matches(&relative) {
                bind.attach(Item::reading(relative));
            }
        }

        Ok(())
    }
}

#[inline]
pub fn select<P>(pattern: P) -> Select<P>
where P: Pattern + Sync + Send + 'static {
    Select {
        pattern: pattern,
    }
}

#[inline]
pub fn create<P>(path: P) -> Create
where P: Into<PathBuf> {
    Create {
        path: path.into(),
    }
}

pub struct Retain<C>
where C: Fn(&Item) -> bool, C: Sync + Send + 'static {
    condition: C,
}

impl<C> Handle<Bind> for Retain<C>
where C: Fn(&Item) -> bool, C: Sync + Send + 'static {
    fn handle(&self, bind: &mut Bind) -> ::Result<()> {
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

pub struct PooledEach {}

impl PooledEach {
    pub fn new() -> PooledEach {
        PooledEach {}
    }

    pub fn each<H>(&self, handler: H) -> Each<H>
    where H: Handle<Item> + Sync + Send + 'static {
        Each {
            handler: Arc::new(handler),
        }
    }
}

pub fn each<H>(handler: H) -> Each<H>
where H: Handle<Item> + Sync + Send + 'static {
    Each {
        handler: Arc::new(handler),
    }
}

pub struct Each<H>
where H: Handle<Item> + Sync + Send + 'static {
    handler: Arc<H>
}

impl<H> Handle<Bind> for Each<H>
where H: Handle<Item> + Sync + Send + 'static {
    fn handle(&self, bind: &mut Bind) -> ::Result<()> {
        let items = mem::replace(bind.items_mut(), vec![]);
        let futures: Vec<_> = items
            .into_iter()
            .map(|mut item| {
                let handler = self.handler.clone();

                let future = future::lazy(move |_| {
                    match handler.handle(&mut item) {
                        Ok(()) => Box::new(future::ok(item)),
                        Err(e) => Box::new(future::err((e, item))),
                    }
                });

                futures::executor::block_on(futures::executor::spawn_with_handle(future)).unwrap()
            })
            .collect();

        match futures::executor::block_on(future::join_all(futures)) {
            Ok(mut results) => mem::swap(&mut results, bind.items_mut()),
            Err((e, item)) => {
                println!("\nthe following item encountered an error:\n  {:?}\n\n{}\n",
                            item, e);
                return Err(e);
            }
        }

        Ok(())
    }
}

pub fn missing(bind: &mut Bind) -> ::Result<()> {
    println!("missing handler for {}", bind);
    Ok(())
}

pub struct SortBy<F>
where F: Fn(&Item, &Item) -> cmp::Ordering,
      F: Sync + Send + 'static {
    compare: F,
}

pub fn sort_by<F>(compare: F) -> SortBy<F>
where F: Fn(&Item, &Item) -> cmp::Ordering,
      F: Sync + Send + 'static {
    SortBy {
        compare: compare,
    }
}

impl<F> Handle<Bind> for SortBy<F>
where F: Fn(&Item, &Item) -> cmp::Ordering,
      F: Sync + Send + 'static {
    fn handle(&self, bind: &mut Bind) -> ::Result<()> {
        bind.items_mut().sort_by(|a, b| -> cmp::Ordering {
            (self.compare)(a, b)
        });

        Ok(())
    }
}

pub struct SortByKey<B, F>
where B: Ord, F: Fn(&Item) -> B,
      F: Sync + Send + 'static {
    key: F,
}

impl<B, F> Handle<Bind> for SortByKey<B, F>
where B: Ord, F: Fn(&Item) -> B,
      F: Sync + Send + 'static {
    fn handle(&self, bind: &mut Bind) -> ::Result<()> {
        bind.items_mut().sort_by_key(|a| {
            (self.key)(a)
        });

        Ok(())
    }
}

pub fn sort_by_key<B, F>(key: F) -> SortByKey<B, F>
where B: Ord, F: Fn(&Item) -> B,
      F: Sync + Send + 'static {
    SortByKey {
        key: key,
    }
}
