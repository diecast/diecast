use std::sync::Arc;
use std::collections::HashSet;
use std::borrow::IntoCow;
use std::convert::AsRef;
use std::path::{Path, PathBuf};

use pattern::Pattern;
use binding;

pub enum Operation {
    Creating(PathBuf),
    Matching(Box<Pattern>),
}

// TODO: optimization: Arc<String> ?
pub struct Rule {
    name: String,
    operation: Operation,
    compiler: Option<Arc<Box<binding::Handler + Sync + Send>>>,
    dependencies: HashSet<String>,
}

impl Rule {
    pub fn creating<'a, P: ?Sized, S: IntoCow<'a, str>>(name: S, path: &P) -> Rule
    where P: AsRef<Path> {
        Rule {
            name: name.into_cow().into_owned(),
            // Into<PathBuf>
            operation: Operation::Creating(path.as_ref().to_path_buf()),
            compiler: None,
            dependencies: HashSet::new(),
        }
    }

    pub fn matching<'a, P, S: IntoCow<'a, str>>(name: S, pattern: P) -> Rule
    where P: Pattern + 'static {
        Rule {
            name: name.into_cow().into_owned(),
            operation: Operation::Matching(Box::new(pattern)),
            compiler: None,
            dependencies: HashSet::new(),
        }
    }

    pub fn compiler<H>(mut self, compiler: H) -> Rule
    where H: binding::Handler + Sync + Send + 'static {
        self.compiler = Some(Arc::new(Box::new(compiler)));
        self
    }

    pub fn get_compiler(&self) -> &Option<Arc<Box<binding::Handler + Sync + Send + 'static>>> {
        &self.compiler
    }

    pub fn depends_on<R: ?Sized>(mut self, dependency: &R) -> Rule
    where R: Register {
        dependency.register(&mut self.dependencies);

        return self;
    }

    // accessors
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn dependencies(&self) -> &HashSet<String> {
        &self.dependencies
    }

    pub fn operation(&self) -> &Operation {
        &self.operation
    }
}

pub trait Register {
    fn register(&self, dependencies: &mut HashSet<String>);
}

impl Register for Rule {
    fn register(&self, dependencies: &mut HashSet<String>) {
        dependencies.insert(self.name.clone());
    }
}

// impl<R> Register for R where R: Register {
//     fn register(&self, dependencies: &mut HashSet<String>) {
//         (**self).register(dependencies);
//     }
// }

// TODO: this has potential for adding string many times despite being the same
// each having diff ref-count
impl Register for str {
    fn register(&self, dependencies: &mut HashSet<String>) {
        dependencies.insert(self.to_string());
    }
}

impl<'a, I> Register for &'a [I] where I: Register {
    fn register(&self, dependencies: &mut HashSet<String>) {
        for i in *self {
            i.register(dependencies);
        }
    }
}

