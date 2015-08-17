use std::sync::Arc;
use std::any::Any;
use std::path::PathBuf;

use typemap;
use syncbox::{ThreadPool, TaskBox};
use eventual::{
    Future,
    Async,
    AsyncError,
    join,
    defer
};

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
        use support;

        let paths = bind.extensions.read().unwrap().get::<InputPaths>().unwrap().clone();

        for path in paths.iter() {
            let relative =
                support::path_relative_from(path, &bind.configuration.input).unwrap()
                .to_path_buf();

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

pub struct PooledEach {
    pool: Option<ThreadPool<Box<TaskBox>>>,
}

impl PooledEach {
    pub fn new(pool: ThreadPool<Box<TaskBox>>) -> PooledEach {
        PooledEach {
            pool: Some(pool),
        }
    }

    pub fn each<H>(&self, handler: H) -> Each<H>
    where H: Handle<Item> + Sync + Send + 'static {
        Each {
            pool: self.pool.clone(),
            handler: Arc::new(handler),
        }
    }
}

pub fn each<H>(handler: H) -> Each<H>
where H: Handle<Item> + Sync + Send + 'static {
    Each {
        handler: Arc::new(handler),
        pool: None,
    }
}

// TODO: should the chunk be in configuration or a parameter?
pub struct Each<H>
where H: Handle<Item> + Sync + Send + 'static {
    handler: Arc<H>,
    pool: Option<ThreadPool<Box<TaskBox>>>
}

impl<H> Each<H>
where H: Handle<Item> + Sync + Send + 'static {
    pub fn threads(mut self, pool: ThreadPool<Box<TaskBox>>) -> Each<H> {
        self.pool = Some(pool);
        self
    }
}

impl<H> Handle<Bind> for Each<H>
where H: Handle<Item> + Sync + Send + 'static {
    fn handle(&self, bind: &mut Bind) -> ::Result<()> {
        if let Some(ref pool) = self.pool {
            let items = ::std::mem::replace(bind.items_mut(), vec![]);
            let futures: Vec<Future<Item, ::Error>> =
                items.into_iter()
                .map(|mut item| {
                    let handler = self.handler.clone();

                    let future = Future::<Item, ::Error>::lazy(move || {
                        match handler.handle(&mut item) {
                            Ok(()) => {
                                Ok(item)
                            },
                            Err(e) => {
                                println!("\nthe following item encountered an error:\n  {:?}\n\n{}\n",
                                         item,
                                         e);
                                Err(e)
                            }
                        }
                    });

                    defer(pool.clone(), future)
                }).collect();

            let items: Vec<Item> = match join(futures).await() {
                Ok(items) => items,
                Err(AsyncError::Failed(e)) => return Err(e),
                Err(AsyncError::Aborted) => return Err(From::from("Future aborted")),
            };

            *bind.items_mut() = items;
        } else {
            for item in bind.iter_mut() {
                match self.handler.handle(item) {
                    Ok(()) => (),
                    Err(e) => {
                        println!(
                            "\nthe following item encountered an error:\n {:?}\n\n{}\n",
                            item,
                            e);
                        return Err(e);
                    }
                }
            }

            return Ok(());
        }

        Ok(())
    }
}

pub fn missing(bind: &mut Bind) -> ::Result<()> {
    println!("missing handler for {}", bind);
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
    fn handle(&self, bind: &mut Bind) -> ::Result<()> {
        bind.items_mut().sort_by(|a, b| -> ::std::cmp::Ordering {
            (self.compare)(a, b)
        });

        Ok(())
    }
}

