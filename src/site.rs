//! Site generation.

use std::sync::Arc;
use std::collections::HashSet;
use std::fs;

// use threadpool::job::Pool;

use pattern::Pattern;
use job;
use item::Item;
use binding::Bind;
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

        // TODO
        // this is complicated because after finding the items of a rule
        // and constructing its binding, the rule.matches(cb) method
        // can be used to perform logic based on the matches (binding)
        // this can have the effect of creating other rules, which would
        // modify the rules which were just iterated
        // for rule in &rules {
        //     let mut bind = Bind::new(rule.name.clone());

        //     match rule.kind {
        //         rule::Kind::Creating(ref path) => {
        //             let conf = self.configuration.clone();

        //             bind.push(Item::to(path.clone(), conf));
        //         },
        //         rule::Kind::Matching(ref pattern) => {
        //             for path in &paths {
        //                 let relative =
        //                     path.relative_from(&self.configuration.input).unwrap()
        //                     .to_path_buf();

        //                 let conf = self.configuration.clone();

        //                 if pattern.matches(&relative) {
        //                     bind.push(Item::from(relative, conf));
        //                 }
        //             }
        //         },
        //     }

        //     rules.extend(rule.callback(&bind).into_iter());

        //     // run matches callback here?
        //     // perhaps the callback passed to from_matches
        //     // should return a vector of rules?
        //     // rule.from_matches(|bind: &Bind| -> Vec<Rule> {})

        //     self.manager.add(bind, rule.compiler.clone(), &rule.dependencies);
        // }

        // TODO
        // this should probably be a recursive function that keeps
        // processing the rules, or an iterative-stack solution
        enum LifeTime {
            Dynamic,
            Static,
        }

        let mut collected = Vec::new();
        let mut rules = rules.into_iter().map(|r| (LifeTime::Static, r)).collect::<Vec<(LifeTime, Rule)>>();

        while let Some((life, rule)) = rules.pop() {
            let mut bind = Bind::new(rule.name().to_string(), self.configuration.clone());
            let data = bind.data.clone();

            // TODO
            // this should be able to go into its own method on Rule?
            match *rule.operation() {
                rule::Operation::Creating(ref path) => {
                    let conf = self.configuration.clone();

                    bind.push(Item::to(path.clone(), data.clone()));
                },
                rule::Operation::Matching(ref pattern) => {
                    for path in &paths {
                        let relative =
                            path.relative_from(&self.configuration.input).unwrap()
                            .to_path_buf();

                        let conf = self.configuration.clone();

                        if pattern.matches(&relative) {
                            bind.push(Item::from(relative, data.clone()));
                        }
                    }
                },
            }

            // add any potential rules returned from the callback
            if let &Some(ref callback) = rule.callback() {
                rules.extend(
                    callback(&bind).into_iter()
                    .map(|r| (LifeTime::Dynamic, r.depends_on(&rule))));
            }

            // TODO: should handle compiler option clone
            self.manager.add(bind, &rule);

            // only save the static rules
            if let LifeTime::Static = life {
                collected.push(rule);
            }
        }

        // for rule in &self.rules {
        //     let mut bind = Bind::new(rule.name.clone());

        //     match rule.kind {
        //         rule::Kind::Creating(ref path) => {
        //             let conf = self.configuration.clone();

        //             bind.push(Item::to(path.clone(), conf));
        //         },
        //         rule::Kind::Matching(ref pattern) => {
        //             for path in &paths {
        //                 let relative =
        //                     path.relative_from(&self.configuration.input).unwrap()
        //                     .to_path_buf();

        //                 let conf = self.configuration.clone();

        //                 if pattern.matches(&relative) {
        //                     bind.push(Item::from(relative, conf));
        //                 }
        //             }
        //         },
        //     }

        //     // add any potential rules returned from the callback
        //     rules.extend(rule.callback(&bind).into_iter());

        //     // run matches callback here?
        //     // perhaps the callback passed to from_matches
        //     // should return a vector of rules?
        //     // rule.from_matches(|bind: &Bind| -> Vec<Rule> {})

        //     self.manager.add(bind, rule.compiler.clone(), &rule.dependencies);
        //     collected.push(rule);
        // }

        // TODO
        // this isn't supposed to add the ones created from_matches
        // since those rules are subject to change based on the matched binding
        mem::replace(&mut self.rules, collected);
    }

    pub fn build(&mut self) {
        // TODO: clean out the output directory here to avoid cruft and conflicts
        trace!("cleaning out directory");
        self.clean();

        trace!("finding jobs");
        self.find_jobs();

        trace!("creating output directory at {:?}", &self.configuration.output);

        // TODO: need a way to determine if there are no jobs
        // create the output directory
        // don't unwrap to ignore "already exists" error
        if let Some(path) = self.configuration.output.parent() {
            if let Some("") = path.to_str() {
                fs::create_dir(&self.configuration.output);
            }
        } else {
            fs::create_dir_all(&self.configuration.output).unwrap();
        }

        // TODO: use resolve_from for partial builds?
        trace!("resolving graph");

        self.manager.execute();
    }

    pub fn register(&mut self, rule: Rule) {
        println!("registering {}", rule.name());

        if !rule.dependencies().is_empty() {
            let names = self.rules.iter().map(|r| r.name().to_string()).collect();
            let diff: HashSet<_> = rule.dependencies().difference(&names).cloned().collect();

            if !diff.is_empty() {
                println!("`{}` depends on unregistered rule(s) `{:?}`", rule.name(), diff);
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

