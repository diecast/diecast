use docopt::{self, Docopt};
use configuration::Configuration;
use std::path::PathBuf;
use std::fs::PathExt;
use std::fs::{
    read_dir,
    remove_dir_all,
    remove_file,
};

use command::Command;
use site::Site;

#[derive(RustcDecodable, Debug)]
struct Options {
    flag_verbose: bool,
    flag_ignore_hidden: bool,
}

static USAGE: &'static str = "
Usage:
    diecast clean [options]

Options:
    -h, --help            Print this message
    -v, --verbose         Use verbose output
    -i, --ignore-hidden   Don't clean out hidden files and directories

This removes the output directory.
";

pub struct Clean {
    target: PathBuf,
    ignore_hidden: bool,
}

impl Clean {
    pub fn new(configuration: &mut Configuration) -> Clean {
        // 1. merge options into configuration; options overrides config
        // 2. construct site from configuration
        // 3. build site

        let docopt =
            Docopt::new(USAGE)
                .unwrap_or_else(|e| e.exit())
                .help(true);

        let help_error = docopt::Error::WithProgramUsage(
            Box::new(docopt::Error::Help),
            USAGE.to_string());

        let options: Options = docopt.decode().unwrap_or_else(|e| {
            e.exit();
        });

        Clean {
            target: configuration.output.clone(),
            ignore_hidden: options.flag_ignore_hidden,
        }
    }
}

impl Command for Clean {
    fn run(&self, _site: Site) {
        println!("removing {:?}", self.target);

        if !self.target.exists() {
            println!("No directory to remove");
        }

        // TODO: maybe obey .gitignore?
        // clear directory
        if !self.ignore_hidden {
            remove_dir_all(&self.target);
        } else {
            for child in read_dir(&self.target).unwrap() {
                let path = child.unwrap().path();

                if path.file_name().unwrap().to_str().unwrap().char_at(0) != '.' {
                    if path.is_dir() {
                        remove_dir_all(&path).unwrap();
                    } else {
                        remove_file(&path);
                    }
                }
            }
        }
    }
}
