use std::path::{PathBuf, AsPath};

use compiler::Compiler;
use pattern::Pattern;

// need:
//
// * P: Pattern
// * PathBuf

pub enum Kind {
    Creating(PathBuf),
    Matching(Box<Pattern>),
}

pub struct Rule {
    pub name: &'static str,
    pub kind: Kind,
    pub compiler: Compiler,
    pub dependencies: Vec<&'static str>,
}

impl Rule {
    pub fn matching<P>(name: &'static str, pattern: P, compiler: Compiler) -> Rule
    where P: Pattern + 'static {
        Rule {
            name: name,
            kind: Kind::Matching(Box::new(pattern)),
            compiler: compiler,
            dependencies: vec![],
        }
    }

    pub fn creating<P: ?Sized>(name: &'static str, path: &P, compiler: Compiler) -> Rule
    where P: AsPath {
        Rule {
            name: name,
            kind: Kind::Creating(path.as_path().to_path_buf()),
            compiler: compiler,
            dependencies: vec![],
        }
    }

    pub fn depends_on<D>(mut self, dependency: D) -> Rule where D: Dependency {
        self.dependencies.push(dependency.name());

        return self;
    }
}

pub trait Dependency {
    fn name(&self) -> &'static str;
}

impl Dependency for &'static str {
    fn name(&self) -> &'static str {
        self.clone()
    }
}

impl<'a> Dependency for &'a Rule {
    fn name(&self) -> &'static str {
        self.name.clone()
    }
}

