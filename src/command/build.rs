use docopt::Docopt;

use site::Site;
use configuration::Configuration;
use command::Command;
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

pub struct Build {
    site: Site,
}

impl Build {
    pub fn new(rules: Vec<Rule>, mut configuration: Configuration) -> Build {
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
            configuration.threads = jobs;
        }

        configuration.is_verbose = options.flag_verbose;

        Build {
            site: Site::new(rules, configuration),
        }
    }

    pub fn plugin(rules: Vec<Rule>, configuration: Configuration) -> Box<Command> {
        Box::new(Build::new(rules, configuration))
    }
}

impl Command for Build {
    fn run(&mut self) {
        self.site.build();
    }
}
