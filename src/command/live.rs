use docopt::{self, Docopt};
use site::Site;
use configuration::Configuration;

use std::old_io::TempDir;
use std::path::PathBuf;

#[derive(RustcDecodable, Debug)]
struct Options {
    flag_jobs: Option<u32>,
    flag_verbose: bool,
}

static USAGE: &'static str = "
Usage:
    diecast live [options]

Options:
    -h, --help          Print this message
    -j N, --jobs N      Number of jobs to run in parallel
    -v, --verbose       Use verbose output
";

pub fn configure(configuration: &mut Configuration) {
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

    // PROBLEM:
    // this breaks because TempDir needs to live for the lifetime of the command
    // this would mean that Command must be a trait, so we can have something like:
    //
    // trait Command {
    //   fn configure(&mut self, configuration: &mut Configuration);
    //   fn run(&mut self, site: Site);
    // }
    //
    // struct Live {
    //   temp_dir: TempDir,
    // }
    //
    // let mut live = Live::new(&mut configuration);
    // live.run();
    //
    // so Diecast would have to store the Command somehow
    //
    // struct Diecast<C> where C: Command {
    //   command: C,
    // }
    //
    let output = TempDir::new(configuration.output.file_name().unwrap().to_str().unwrap()).unwrap();

    println!("output dir: {:?}", output.path());

    configuration.output = PathBuf::new(output.path().as_str().unwrap());
}

pub fn run(mut site: Site) {
    loop {
        println!("waiting for notifications");
        // block until changes
        // get_notification(

        // rebuild site
        site.build();
        break;
    }
}
