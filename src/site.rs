//! Site generation.

use std::sync::Arc;
use std::collections::HashSet;

use job;
use configuration::Configuration;
use rule::Rule;
use support;

/// A Site scans the input path to find
/// files that match the given pattern. It then
/// takes each of those files and passes it through
/// the compiler chain.
pub struct Site {
    configuration: Configuration,
    rules: Vec<Arc<Rule>>,
}

impl Site {
    pub fn new(rules: Vec<Rule>) -> Site {
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
            configuration: Configuration::new(),
            rules: site_rules,
        }
    }

    pub fn build(&mut self) -> ::Result<()> {
        try!(self.clean());

        let mut manager = job::Manager::new(Arc::new(self.configuration.clone()));

        println!("building from {:?}", self.configuration.input);

        if !support::file_exists(&self.configuration.input) {
            println!("the input directory `{:?}` does not exist!", self.configuration.input);
            ::std::process::exit(1);
        }

        manager.update_paths();

        for rule in &self.rules {
           // FIXME: this just seems weird re: strings
           manager.add(rule.clone());
        }

        // create the output directory
        support::mkdir_p(&self.configuration.output).unwrap();

        manager.build()
    }

    pub fn configuration(&self) -> &Configuration {
        &self.configuration
    }

    pub fn configuration_mut(&mut self) -> &mut Configuration {
        &mut self.configuration
    }

    pub fn clean(&self) -> ::Result<()> {
        use std::fs::{
            read_dir,
            remove_dir_all,
            remove_file,
        };

        // output directory doesn't even exist; nothing to clean
        if !support::file_exists(&self.configuration.output) {
            return Ok(());
        }

        // TODO: probably don't need ignore hidden?
        // TODO: maybe obey .gitignore?
        // clear directory
        for child in try!(read_dir(&self.configuration.output)) {
            let child = try!(child);
            let path = child.path();

            let is_hidden =
                path.file_name()
                .map_or(false, |name|
                     name.to_str()
                     .map_or(false, |s|
                          s.chars().next().map_or(false, |c| c != '.')));

            if !self.configuration.ignore_hidden || is_hidden {
                if ::std::fs::metadata(&path).unwrap().is_dir() {
                    remove_dir_all(&path).unwrap();
                } else {
                    remove_file(&path).unwrap();
                }
            }
        }

        Ok(())
    }
}
