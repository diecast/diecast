//! Site generation.

use std::sync::Arc;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;

// use threadpool::job::Pool;

use pattern::Pattern;
use job::{self, Job};
use compiler::{self, Compile};
use item::Item;
use dependency::Graph;
use configuration::Configuration;
use rule::{self, Rule};

use std::path::{PathBuf, Path};
use std::mem;

/// A Site scans the input path to find
/// files that match the given pattern. It then
/// takes each of those files and passes it through
/// the compiler chain.
pub struct Site {
    configuration: Arc<Configuration>,
    rules: Vec<Rule>,
    manager: job::Manager,
}

impl Site {
    pub fn new(configuration: Configuration) -> Site {
        trace!("output directory is: {:?}", configuration.output);

        let manager = job::Manager::new(configuration.threads);
        let configuration = Arc::new(configuration);

        Site {
            configuration: configuration,
            rules: Vec::new(),
            manager: manager,
        }
    }
}

impl Site {
    // TODO: make this generate a Vec<Job> which is then sent to the manager
    pub fn find_jobs(&mut self) {
        use std::fs::PathExt;

        let rules = mem::replace(&mut self.rules, Vec::new());

        let paths =
            fs::walk_dir(&self.configuration.input).unwrap()
            .filter_map(|p| {
                let path = p.unwrap().path();

                if let Some(ref pattern) = self.configuration.ignore {
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

        for rule in &rules {
            let mut binding: Vec<Item> = vec![];

            match rule.kind {
                rule::Kind::Creating(ref path) => {
                    let compiler = rule.compiler.clone();
                    let conf = self.configuration.clone();

                    binding.push(Item::new(conf, None, Some(path.clone())));
                },
                rule::Kind::Matching(ref pattern) => {
                    for path in &paths {
                        let relative =
                            path.relative_from(&self.configuration.input)
                            .unwrap()
                            .to_path_buf();

                        let conf = self.configuration.clone();

                        if pattern.matches(&relative) {
                            binding.push(Item::new(conf, Some(relative), None));
                        }
                    }
                },
            }

            self.manager.add(rule.name, rule.compiler.clone(), &rule.dependencies, binding);
        }

        mem::replace(&mut self.rules, rules);
    }

    pub fn build(&mut self) {
        // TODO: clean out the output directory here to avoid cruft and conflicts
        trace!("cleaning out directory");
        self.clean();

        trace!("finding jobs");
        self.find_jobs();

        // TODO: need a way to determine if there are no jobs
        // create the output directory
        fs::create_dir_all(&self.configuration.output).unwrap();

        // TODO: use resolve_from for partial builds?
        trace!("resolving graph");

        self.manager.execute();
    }

    pub fn bind(&mut self, rule: Rule) {
        for &dependency in &rule.dependencies {
            if !self.rules.iter().any(|r| r.name == dependency) {
                println!("`{}` depends on unregistered rule `{}`",
                         rule.name,
                         dependency);
                ::exit(1);
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

