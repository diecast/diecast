use docopt::{self, Docopt};
use site::Configuration;

#[derive(RustcDecodable, Debug)]
struct Options {
    flag_jobs: Option<u32>,
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

pub fn execute() {
    // 1. merge options into configuration; options overrides config
    // 2. construct site from configuration
    // 3. build site

    for arg in ::std::env::args() {
        print!("{:?} ", arg);
    }

    println!("");

    let docopt =
        Docopt::new(USAGE)
            .unwrap_or_else(|e| e.exit())
            .help(true);

    let help_error = docopt::Error::WithProgramUsage(
        Box::new(docopt::Error::Help),
        USAGE.to_string());

    let options: Options = docopt.decode().unwrap_or_else(|e| {
        if let docopt::Error::WithProgramUsage(ref error, _) = e {
            if let &docopt::Error::Help = &**error {
                e.exit();
            }
        }

        println!("\nCouldn't parse arguments");
        help_error.exit();
    });

    println!("{:?}", options);
}
