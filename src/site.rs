//! Site generation.

use std::sync::Arc;
use std::collections::HashSet;
use std::fs;

use job::{self, Job};
use binding::Bind;
use configuration::Configuration;
use rule::Rule;

/// A Site scans the input path to find
/// files that match the given pattern. It then
/// takes each of those files and passes it through
/// the compiler chain.
pub struct Site {
    configuration: Arc<Configuration>,
    rules: Vec<Rule>,
    // manager: job::Manager<VecDeque<job::Job>>,
    manager: job::Manager<job::evaluator::Pool<Job>>,
}

impl Site {
    pub fn new(configuration: Configuration) -> Site {
        trace!("output directory is: {:?}", configuration.output);

        // let queue: VecDeque<job::Job> = VecDeque::new();
        let queue = job::evaluator::Pool::new(4);

        let manager = job::Manager::new(queue);
        let configuration = Arc::new(configuration);

        Site {
            configuration: configuration,
            rules: Vec::new(),
            manager: manager,
        }
    }
}

impl Site {
    pub fn build(&mut self) {
        // TODO: clean out the output directory here to avoid cruft and conflicts
        trace!("cleaning out directory");
        self.clean();

        trace!("finding jobs");

        for rule in &self.rules {
            // FIXME: this just seems weird re: strings
            self.manager.add(&rule, Bind::new(String::from(rule.name()), self.configuration.clone()));
        }

        trace!("creating output directory at {:?}", &self.configuration.output);

        // TODO: need a way to determine if there are no jobs
        // create the output directory
        // don't unwrap to ignore "already exists" error
        // FIXME: do and_then
        if let Some(path) = self.configuration.output.parent() {
            if let Some("") = path.to_str() {
                fs::create_dir(&self.configuration.output);
            }
        } else {
            ::mkdir_p(&self.configuration.output).unwrap();
        }

        // TODO: use resolve_from for partial builds?
        trace!("resolving graph");

        self.manager.execute();
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

        self.rules.push(rule);
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

        if !self.configuration.output.exists() {
            return;
        }

        // TODO: maybe obey .gitignore?
        // clear directory
        for child in read_dir(&self.configuration.output).unwrap() {
            let path = child.unwrap().path();

            if !self.configuration.ignore_hidden ||
                path.file_name().unwrap().to_str().unwrap().char_at(0) != '.' {
                if path.is_dir() {
                    remove_dir_all(&path).unwrap();
                } else {
                    remove_file(&path).unwrap();
                }
            }
        }
    }
}

