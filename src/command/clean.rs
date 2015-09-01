use std::error::Error;

use docopt::Docopt;

use command::Command;
use configuration::Configuration;
use site::Site;
use support;

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

pub struct Clean;

impl Clean {
    pub fn configure(&mut self, configuration: &mut Configuration) {
        let docopt =
            Docopt::new(USAGE)
                .unwrap_or_else(|e| e.exit())
                .help(true);

        let options: Options = docopt.decode().unwrap_or_else(|e| {
            e.exit();
        });

        configuration.ignore_hidden = options.flag_ignore_hidden;
    }
}

impl Command for Clean {
    fn description(&self) -> &'static str {
        "Remove output directory"
    }

    fn run(&mut self, site: &mut Site) -> ::Result<()> {
        let target = &site.configuration().output;

        if support::file_exists(target) {
            println!("removing {:?}", target);
        } else {
            println!("nothing to remove");
        }

        // TODO: clean return Result?
        site.clean()
    }
}
