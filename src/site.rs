//! Site generation.

use std::sync::Arc;
use std::path::PathBuf;
use std::collections::HashSet;

use job::{self, Job};
use configuration::Configuration;
use rule::Rule;

/// A Site scans the input path to find
/// files that match the given pattern. It then
/// takes each of those files and passes it through
/// the compiler chain.
pub struct Site {
    configuration: Arc<Configuration>,
    rules: Vec<Arc<Rule>>,
    // manager: job::Manager<VecDeque<job::Job>>,
    manager: job::Manager<job::evaluator::Pool<Job>>,
}

impl Site {
    pub fn new(configuration: Configuration) -> Site {
        trace!("output directory is: {:?}", configuration.output);

        // let queue: VecDeque<job::Job> = VecDeque::new();
        let queue = job::evaluator::Pool::new(4);

        let configuration = Arc::new(configuration);
        let manager = job::Manager::new(queue, configuration.clone());

        Site {
            configuration: configuration,
            rules: Vec::new(),
            manager: manager,
        }
    }
}

impl Site {
    fn prepare(&mut self) {
        trace!("finding jobs");

        for rule in &self.rules {
           // FIXME: this just seems weird re: strings
           self.manager.add(rule.clone());
        }

        trace!("creating output directory at {:?}", &self.configuration.output);

        // create the output directory
        ::mkdir_p(&self.configuration.output).unwrap();

        // TODO: use resolve_from for partial builds?
        trace!("resolving graph");
    }

    pub fn build(&mut self) {
        // TODO: clean out the output directory here to avoid cruft and conflicts
        // trace!("cleaning out directory");
        self.clean();

        self.prepare();
        self.manager.build();
    }

    pub fn update(&mut self, paths: HashSet<PathBuf>) {
        self.prepare();
        self.manager.update(paths);
    }

    pub fn register(&mut self, rule: Rule) {
        if !rule.dependencies().is_empty() {
            let names = self.rules.iter().map(|r| String::from(r.name())).collect();
            let diff: HashSet<_> = rule.dependencies().difference(&names).cloned().collect();

            if !diff.is_empty() {
                println!("`{}` depends on unregistered rule(s) `{:?}`", rule.name(), diff);
                ::std::process::exit(1);
            }
        }

        self.rules.push(Arc::new(rule));
    }

    pub fn configuration(&self) -> Arc<Configuration> {
        self.configuration.clone()
    }

    pub fn clean(&self) {
        use std::fs::PathExt;
        use std::fs::{
            read_dir,
            remove_dir_all,
            remove_file,
        };

        trace!("cleaning");

        if !self.configuration.output.exists() {
            return;
        }

        // TODO: maybe obey .gitignore?
        // clear directory
        for child in read_dir(&self.configuration.output).unwrap() {
            let path = child.unwrap().path();

            if !self.configuration.ignore_hidden ||
                path.file_name().unwrap()
                    .to_str().unwrap()
                    .chars().next().unwrap() != '.' {
                if path.is_dir() {
                    remove_dir_all(&path).unwrap();
                } else {
                    remove_file(&path).unwrap();
                }
            }
        }
    }
}

