use std::sync::Arc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;

use binding;
use item::{Item, Route};
use source::Source;
use pattern::Pattern;
use util;

pub struct Create {
    path: PathBuf,
}

impl Source for Create {
    fn source(&self, bind: Arc<binding::Data>) -> Vec<Item> {
        vec![Item::new(Route::Write(self.path.clone()), bind.clone())]
    }
}

#[inline]
pub fn create(path: PathBuf) -> Create {
    Create {
        path: path,
    }
}

pub struct Select<P>
where P: Pattern + Sync + Send + 'static {
    pattern: P,
}

impl<P> Source for Select<P>
where P: Pattern + Sync + Send + 'static {
    fn source(&self, bind: Arc<binding::Data>) -> Vec<Item> {
        use std::fs::PathExt;

        let mut items = vec![];

        let paths =
            fs::walk_dir(&bind.configuration.input).unwrap()
            .filter_map(|p| {
                let path = p.unwrap().path();

                if let Some(ref pattern) = bind.configuration.ignore {
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
                path.relative_from(&bind.configuration.input).unwrap()
                .to_path_buf();

            // TODO: JOIN STANDARDS
            // should insert path.clone()
            if self.pattern.matches(&relative) {
                items.push(Item::new(Route::Read(relative), bind.clone()));
            }
        }

        items
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

pub struct Paginate<R>
where R: Fn(usize) -> PathBuf, R: Sync + Send + 'static {
    target: String,
    factor: usize,
    router: R
}

impl<R> Source for Paginate<R>
where R: Fn(usize) -> PathBuf, R: Sync + Send + 'static {
    fn source(&self, bind: Arc<binding::Data>) -> Vec<Item> {
        pages(bind.dependencies[&self.target].items().len(), self.factor, &self.router, bind)
    }
}

// FIXME
// the problem with this using indices is that if the bind is sorted
// or the order is otherwise changed, the indices will no longer match!
pub fn pages<R>(input: usize, factor: usize, router: &R, bind: Arc<binding::Data>) -> Vec<Item>
where R: Fn(usize) -> PathBuf, R: Sync + Send + 'static {
    let mut items = vec![];
    let post_count = input;

    let page_count = {
        let (div, rem) = (post_count / factor, post_count % factor);

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
            .or_insert_with(|| Arc::new(router(num)))
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

        let start = current * factor;
        let end = ::std::cmp::min(post_count, (current + 1) * factor);

        let target = router(current);

        let first = first.clone();
        let last = last.clone();
        let curr = (current, target.clone());

        let page_struct =
            util::handle::item::Page {
                first: first,

                prev: prev,
                curr: curr,
                next: next,

                last: last,

                page_count: page_count,
                post_count: post_count,
                posts_per_page: factor,

                range: start .. end,
            };

        let mut page = Item::new(Route::Write((*target).clone()), bind.clone());
        page.extensions.insert::<util::handle::item::Page>(page_struct);
        items.push(page);
    }

    items
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

pub fn none(_bind: Arc<binding::Data>) -> Vec<Item> {
    vec![]
}
