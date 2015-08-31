use std::error::Error;

use docopt::Docopt;

use command::{Command, Plugin};
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

pub fn plugin() -> Plugin {
    Plugin::new("clean", "Remove output directory", Clean::plugin)
}

pub struct Clean {
    site: Site,
}

impl Clean {
    pub fn new(mut site: Site) -> Clean {
        let docopt =
            Docopt::new(USAGE)
                .unwrap_or_else(|e| e.exit())
                .help(true);

        let options: Options = docopt.decode().unwrap_or_else(|e| {
            e.exit();
        });

        site.configuration_mut().ignore_hidden = options.flag_ignore_hidden;

        Clean {
            site: site,
        }
    }

    pub fn plugin(site: Site) -> Box<Command> {
        Box::new(Clean::new(site))
    }
}

impl Command for Clean {
    fn run(&mut self) -> ::Result<()> {
        let target = &self.site.configuration().output;

        if support::file_exists(target) {
            println!("removing {:?}", target);
        } else {
            println!("nothing to remove");
        }

        // TODO: clean return Result?
        self.site.clean()
    }
}
