use std::path::{PathBuf, AsPath};

use compiler::Chain;
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
    pub compiler: Chain,
    pub dependencies: Vec<&'static str>,
}

impl Rule {
    pub fn matching<P>(name: &'static str, pattern: P, compiler: Chain) -> Rule
    where P: Pattern + 'static {
        Rule {
            name: name,
            kind: Kind::Matching(Box::new(pattern)),
            compiler: compiler,
            dependencies: vec![],
        }
    }

    pub fn creating<P: ?Sized>(name: &'static str, path: &P, compiler: Chain) -> Rule
    where P: AsPath {
        Rule {
            name: name,
            kind: Kind::Creating(path.as_path().to_path_buf()),
            compiler: compiler,
            dependencies: vec![],
        }
    }

    pub fn depends_on<'a>(mut self, dependency: &'a Rule) -> Rule {
        self.dependencies.push(dependency.name.clone());

        return self;
    }
}

