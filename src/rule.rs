use std::path::{PathBuf, AsPath};
use std::sync::Arc;

use compiler::Compile;
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
    pub compiler: Arc<Box<Compile>>,
    pub dependencies: Vec<&'static str>,
}

impl Rule {
    pub fn matching<P, C>(name: &'static str, pattern: P, compiler: C) -> Rule
    where P: Pattern + 'static, C: Compile + 'static {
        Rule {
            name: name,
            kind: Kind::Matching(Box::new(pattern)),
            compiler: Arc::new(Box::new(compiler)),
            dependencies: vec![],
        }
    }

    pub fn creating<P: ?Sized, C>(name: &'static str, path: &P, compiler: C) -> Rule
    where P: AsPath, C: Compile + 'static {
        Rule {
            name: name,
            kind: Kind::Creating(path.as_path().to_path_buf()),
            compiler: Arc::new(Box::new(compiler)),
            dependencies: vec![],
        }
    }

    pub fn depends_on<'a>(mut self, dependency: &'a Rule) -> Rule {
        self.dependencies.push(dependency.name.clone());

        return self;
    }
}

