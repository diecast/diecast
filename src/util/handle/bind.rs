use std::sync::Arc;
use std::any::Any;
use std::path::PathBuf;

use typemap;

use job::evaluator::Pool;
use item::Item;
use bind::Bind;
use handler::Handle;

use super::Extender;

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

// TODO
// should this probably be a separate crate?
// store mutex<threadpool> in extensions,
// then this handler would use it?
pub fn each<H>(handler: H) -> Each<H>
where H: Handle<Item> + Sync + Send + 'static {
    Each {
        chunk: 1,
        handler: Arc::new(handler),
        threads: 1,
    }
}

// TODO: should the chunk be in configuration or a parameter?
pub struct Each<H>
where H: Handle<Item> + Sync + Send + 'static {
    // TODO remove chunk
    chunk: usize,
    handler: Arc<H>,
    threads: usize,
}

impl<H> Each<H>
where H: Handle<Item> + Sync + Send + 'static {
    pub fn chunk(mut self, size: usize) -> Each<H> {
        self.chunk = size;
        self
    }

    pub fn threads(mut self, threads: usize) -> Each<H> {
        self.threads = threads;
        self
    }
}

impl<H> Handle<Bind> for Each<H>
where H: Handle<Item> + Sync + Send + 'static {
    fn handle(&self, bind: &mut Bind) -> ::Result<()> {
        if self.threads == 1 {
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

        let pool: Pool<Item> = Pool::new(self.threads);
        let total = bind.items().len();

        let items = ::std::mem::replace(bind.items_mut(), vec![]);

        for mut item in items {
            let handler = self.handler.clone();

            pool.enqueue(move || {
                match <Handle<Item>>::handle(&handler, &mut item) {
                    Ok(()) => Some(item),
                    Err(e) => {
                        println!("\nthe following item encountered an error:\n  {:?}\n\n{}\n",
                                 item,
                                 e);
                        return None;
                    }
                }
            });
        }

        for _ in 0 .. total {
            // TODO: this completely defeats the purpose of hiding the items field
            bind.items_mut().push(pool.dequeue().unwrap());
        }

        assert_eq!(total, bind.items().len());

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

