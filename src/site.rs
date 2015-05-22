//! Site generation.

use std::sync::Arc;
use std::path::PathBuf;
use std::collections::HashSet;

use job::{self, Job};
use configuration::Configuration;
use rule::Rule;
use support;

/// A Site scans the input path to find
/// files that match the given pattern. It then
/// takes each of those files and passes it through
/// the compiler chain.
pub struct Site {
    configuration: Arc<Configuration>,
    rules: Vec<Arc<Rule>>,
    manager: job::Manager<job::evaluator::Pool<Job>>,
}

impl Site {
    pub fn new(rules: Vec<Rule>, configuration: Configuration) -> Site {
        let queue = job::evaluator::Pool::new(4);

        let configuration = Arc::new(configuration);
        let manager = job::Manager::new(queue, configuration.clone());

        let mut site_rules = vec![];

        let names =
            rules.iter()
            .map(|r| String::from(r.name()))
            .collect::<HashSet<_>>();

        for rule in rules {
            if !rule.dependencies().is_empty() {
                let diff: HashSet<_> =
                    rule.dependencies().difference(&names).collect();

                if !diff.is_empty() {
                    println!("`{}` depends on unregistered rule(s) `{:?}`", rule.name(), diff);
                    ::std::process::exit(1);
                }
            }

            site_rules.push(Arc::new(rule));
        }

        Site {
            configuration: configuration,
            rules: site_rules,
            manager: manager,
        }
    }

    fn prepare(&mut self) {
        println!("building from {:?}", self.configuration.output);

        if !support::file_exists(&self.configuration.input) {
            println!("the input directory `{:?}` does not exist!", self.configuration.input);
            ::std::process::exit(1);
        }

        self.manager.update_paths();

        for rule in &self.rules {
           // FIXME: this just seems weird re: strings
           self.manager.add(rule.clone());
        }

        // create the output directory
        support::mkdir_p(&self.configuration.output).unwrap();
    }

    pub fn build(&mut self) {
        self.clean();

        self.prepare();
        self.manager.build();
    }

    pub fn update(&mut self, paths: HashSet<PathBuf>) {
        self.prepare();
        self.manager.update(paths);
    }

    pub fn configuration(&self) -> Arc<Configuration> {
        self.configuration.clone()
    }

    pub fn clean(&self) {
        use std::fs::{
            read_dir,
            remove_dir_all,
            remove_file,
        };

        if !support::file_exists(&self.configuration.output) {
            return;
        }

        // TODO: probably don't need ignore hidden?
        // TODO: maybe obey .gitignore?
        // clear directory
        for child in read_dir(&self.configuration.output).unwrap() {
            let path = child.unwrap().path();

            if !self.configuration.ignore_hidden ||
                path.file_name().unwrap()
                    .to_str().unwrap()
                    .chars().next().unwrap() != '.' {
                if ::std::fs::metadata(&path).unwrap().is_dir() {
                    remove_dir_all(&path).unwrap();
                } else {
                    remove_file(&path).unwrap();
                }
            }
        }
    }
}

