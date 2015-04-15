use std::sync::Arc;
use std::path::{PathBuf, Path};
use std::collections::HashMap;
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

pub fn static_file<P>(pattern: P) -> Chain<Bind>
where P: Pattern + Sync + Send + 'static {
    Chain::new()
    .link(select(pattern))
    .link(each(Chain::new()
        .link(::util::route::identity)
        .link(item::copy)))
}

impl<H> Handle<Bind> for Each<H>
where H: Handle<Item> {
    fn handle(&self, binding: &mut Bind) -> Result {
        for item in &mut binding.items {
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
where T: Sync + Send + Clone + 'static {
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
        let item_count = bind.items.len();

        let chunks = {
            let (div, rem) = (item_count / self.chunk, item_count % self.chunk);

            if rem == 0 {
                div
            } else {
                div + 1
            }
        };

        let mut items = bind.items.drain().collect::<Vec<Item>>();

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
            bind.items.extend(pool.dequeue().unwrap().into_iter());
        }

        assert!(item_count == bind.items.len(), "received different number of items from pool");

        Ok(())
    }
}

pub fn stub(_bind: &mut Bind) -> handle::Result {
    trace!("stub handler");
    Ok(())
}

pub struct Retain<C>
where C: Fn(&Item) -> bool, C: Sync + Send + 'static {
    condition: C,
}

impl<C> Handle<Bind> for Retain<C>
where C: Fn(&Item) -> bool, C: Sync + Send + 'static {
    fn handle(&self, bind: &mut Bind) -> handle::Result {
        bind.items.retain(&self.condition);
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

#[derive(Clone, Debug)]
pub struct Adjacent {
    previous: Option<Arc<Item>>,
    next: Option<Arc<Item>>,
}

pub fn next_prev(bind: &mut Bind) -> handle::Result {
    let count = bind.items.len();

    let last_num = if count == 0 {
        0
    } else {
        count - 1
    };

    // TODO: yet another reason to have Arc<Item>?
    let cloned = bind.items.iter().map(|i| Arc::new(i.clone())).collect::<Vec<Arc<Item>>>();

    for (idx, item) in bind.items.iter_mut().enumerate() {
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

pub struct Paginate<R>
where R: Fn(usize) -> PathBuf, R: Sync + Send + 'static {
    target: String,
    factor: usize,
    router: R
}

impl<R> Handle<Bind> for Paginate<R>
where R: Fn(usize) -> PathBuf, R: Sync + Send + 'static {
    fn handle(&self, bind: &mut Bind) -> handle::Result {
        let post_count = bind.data().dependencies[&self.target].items.len();

        let page_count = {
            let (div, rem) = (post_count / self.factor, post_count % self.factor);

            if rem == 0 {
                div
            } else {
                div + 1
            }
        };

        let last_num = page_count - 1;

        let mut cache: HashMap<usize, Arc<PathBuf>> = HashMap::new();

        let mut router = |num: usize| -> Arc<PathBuf> {
            cache.entry(num)
                .or_insert_with(|| Arc::new((self.router)(num)))
                .clone()
        };

        let first = (1, router(1));
        let last = (last_num, router(last_num));

        // grow the number of pages as needed
        for current in 0 .. page_count {
            let prev =
                if current == 0 { None }
                else { let num = current - 1; Some((num, router(num))) };
            let next =
                if current == last_num { None }
                else { let num = current + 1; Some((num, router(num))) };

            let start = current * self.factor;
            let end = ::std::cmp::min(post_count, (current + 1) * self.factor);

            let target = router(current);

            let first = first.clone();
            let last = last.clone();
            let curr = (current, target.clone());

            let page_struct =
                item::Page {
                    first: first,

                    prev: prev,
                    curr: curr,
                    next: next,

                    last: last,

                    page_count: page_count,
                    post_count: post_count,
                    posts_per_page: self.factor,

                    range: start .. end,
                };

            let mut page = bind.spawn(Route::Write((*target).clone()));
            page.extensions.insert::<item::Page>(page_struct);
            bind.items.push(page);
        }

        Ok(())
    }
}

// TODO: this should actually use a Dependency -> name trait
// we probably have to re-introduce it
#[inline]
pub fn paginate<S: Into<String>, R>(target: S, factor: usize, router: R) -> Paginate<R>
where R: Fn(usize) -> PathBuf, R: Sync + Send + 'static {
    Paginate {
        target: target.into(),
        factor: factor,
        router: router,
    }
}

pub struct Select<P>
where P: Pattern + Sync + Send + 'static {
    pattern: P,
}

impl<P> Handle<Bind> for Select<P>
where P: Pattern + Sync + Send + 'static {
    fn handle(&self, bind: &mut Bind) -> handle::Result {
        use std::fs::PathExt;

        let paths =
            fs::walk_dir(&bind.data().configuration.input).unwrap()
            .filter_map(|p| {
                let path = p.unwrap().path();

                if let Some(ref pattern) = bind.data().configuration.ignore {
                    if pattern.matches(&Path::new(path.file_name().unwrap())) {
                        return None;
                    }
                }

                if path.is_file() {
                    Some(path.to_path_buf())
                } else {
                    None
                }
            })
            .collect::<Vec<PathBuf>>();

        for path in &paths {
            let relative =
                path.relative_from(&bind.data().configuration.input).unwrap()
                .to_path_buf();

            // TODO: JOIN STANDARDS
            // should insert path.clone()
            if self.pattern.matches(&relative) {
                bind.push(Route::Read(relative));
            }
        }

        Ok(())
    }
}

// TODO: problem here is that the dir is being walked multiple times
#[inline]
pub fn select<P>(pattern: P) -> Select<P>
where P: Pattern + Sync + Send + 'static {
    Select {
        pattern: pattern,
    }
}

pub struct Create {
    path: PathBuf,
}

impl Handle<Bind> for Create {
    fn handle(&self, bind: &mut Bind) -> handle::Result {
        println!("creating {:?}", self.path);
        bind.push(Route::Write(self.path.clone()));

        Ok(())
    }
}

#[inline]
pub fn create(path: PathBuf) -> Create {
    Create {
        path: path,
    }
}

#[derive(Clone)]
pub struct Tags {
    map: HashMap<String, Vec<Arc<Item>>>,
}

pub fn tags(bind: &mut Bind) -> handle::Result {
    let mut tag_map = ::std::collections::HashMap::new();

    for item in &bind.items {
        let toml =
            item.extensions.get::<super::item::Metadata>()
            .and_then(|m| {
                m.data.lookup("tags")
            })
            .and_then(::toml::Value::as_slice);

        let arc = Arc::new(item.clone());

        if let Some(tags) = toml {
            for tag in tags {
                tag_map.entry(tag.as_str().unwrap().to_string())
                    .or_insert(vec![])
                    .push(arc.clone());
            }
        }
    }

    bind.data().extensions.write().unwrap().insert::<Tags>(Tags { map: tag_map });

    Ok(())
}

pub fn sort_by_date(bind: &mut Bind) -> handle::Result {
    bind.items.sort_by(|a, b| -> ::std::cmp::Ordering {
        let a = a.extensions.get::<chrono::NaiveDate>().unwrap();
        let b = b.extensions.get::<chrono::NaiveDate>().unwrap();
        b.cmp(a)
    });

    println!("sorted: {:?}", bind.items);

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
        bind.items.sort_by(|a, b| -> ::std::cmp::Ordering {
            (self.compare)(a, b)
        });

        println!("sorted: {:?}", bind.items);

        Ok(())
    }
}

pub struct SortByExtension<T, F>
where T: ::std::any::Any + Ord + Clone + Sync + Send + 'static,
      F: Fn(&T, &T) -> ::std::cmp::Ordering,
      F: Sync + Send + 'static {
    compare: F,
    _phantom: ::std::marker::PhantomData<T>,
}

pub fn sort_by_extension<T, F>(compare: F) -> SortByExtension<T, F>
where T: ::std::any::Any + Ord + Clone + Sync + Send + 'static,
      F: Fn(&T, &T) -> ::std::cmp::Ordering,
      F: Sync + Send + 'static {
    SortByExtension {
        compare: compare,
        _phantom: ::std::marker::PhantomData,
    }
}

impl<T, F> Handle<Bind> for SortByExtension<T, F>
where T: ::std::any::Any + Ord + Clone + Sync + Send + 'static,
      F: Fn(&T, &T) -> ::std::cmp::Ordering,
      F: Sync + Send + 'static {
    fn handle(&self, bind: &mut Bind) -> handle::Result {
        bind.items.sort_by(|a, b| -> ::std::cmp::Ordering {
            (self.compare)(a.extensions.get::<T>().unwrap(), b.extensions.get::<T>().unwrap())
        });

        println!("sorted: {:?}", bind.items);

        Ok(())
    }
}

