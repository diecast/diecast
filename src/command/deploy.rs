use std::error::Error;

use docopt::Docopt;

use site::Site;
use command::Command;
use configuration::Configuration;

#[derive(RustcDecodable, Debug)]
struct Options {
    flag_jobs: Option<usize>,
    flag_verbose: bool,
}

static USAGE: &'static str = "
Usage:
    diecast deploy [options]

Options:
    -h, --help          Print this message
    -j N, --jobs N      Number of jobs to run in parallel
    -v, --verbose       Use verbose output
";

pub struct Deploy<P>
where P: Fn(&Site) -> ::Result<()> {
    procedure: P
}

impl<P> Deploy<P>
where P: Fn(&Site) -> ::Result<()> {
    pub fn new(procedure: P) -> Deploy<P> {
        Deploy {
            procedure: procedure,
        }
    }

    pub fn configure(&mut self, configuration: &mut Configuration) {
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
    }
}

impl<P> Command for Deploy<P>
where P: Fn(&Site) -> ::Result<()> {
    fn description(&self) -> &'static str {
        "Deploy the site"
    }

    fn run(&mut self, site: &mut Site) -> ::Result<()> {
        self.configure(site.configuration_mut());
        (self.procedure)(site)
    }
}
