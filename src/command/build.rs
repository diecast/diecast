use std::error::Error;

use docopt::Docopt;

use site::Site;
use configuration::Configuration;
use command::{Command, Plugin};
use rule::Rule;

#[derive(RustcDecodable, Debug)]
struct Options {
    flag_jobs: Option<usize>,
    flag_verbose: bool,
}

static USAGE: &'static str = "
Usage:
    diecast build [options]

Options:
    -h, --help          Print this message
    -j N, --jobs N      Number of jobs to run in parallel
    -v, --verbose       Use verbose output
";

pub fn plugin() -> Plugin {
    Plugin::new("build", "Build the site", Build::plugin)
}

pub struct Build {
    site: Site,
}

impl Build {
    pub fn new(mut site: Site) -> Build {
        // 1. merge options into configuration; options overrides config
        // 2. construct site from configuration
        // 3. build site

        let docopt =
            Docopt::new(USAGE)
                .unwrap_or_else(|e| e.exit())
                .help(true);

        let options: Options = docopt.decode().unwrap_or_else(|e| {
            e.exit();
        });

        if let Some(jobs) = options.flag_jobs {
            site.configuration_mut().threads = jobs;
        }

        site.configuration_mut().is_verbose = options.flag_verbose;

        Build {
            site: site,
        }
    }

    pub fn plugin(site: Site) -> Box<Command> {
        Box::new(Build::new(site))
    }
}

impl Command for Build {
    fn run(&mut self) -> ::Result<()> {
        // TODO: build return Result?
        self.site.build()
    }
}
