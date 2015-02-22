use docopt::Docopt;
use site::Site;
use configuration::Configuration;

use std::old_io::TempDir;
use std::path::PathBuf;

use command::Command;

use notify::{RecommendedWatcher, Error, Watcher};
use std::sync::mpsc::channel;

#[derive(RustcDecodable, Debug)]
struct Options {
    flag_jobs: Option<usize>,
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

pub struct Live {
    temp_dir: TempDir,
}

impl Live {
    pub fn new(configuration: &mut Configuration) -> Live {
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

        let live = Live {
            temp_dir:
                TempDir::new(
                    configuration.output.file_name().unwrap()
                        .to_str().unwrap()).unwrap(),
        };

        configuration.output = PathBuf::new(live.temp_dir.path().as_str().unwrap());
        println!("output dir: {:?}", live.temp_dir.path());

        live
    }
}

impl Command for Live {
    fn run(&self, mut site: Site) {
        let (tx, rx) = channel();
        let mut w: Result<RecommendedWatcher, Error> = Watcher::new(tx);

        match w {
            Ok(mut watcher) => {
                watcher.watch(&site.configuration().input);

                site.build();

                loop {
                    let event = rx.recv().unwrap();
                    site.build();
                }
            },
            Err(e) => println!("Error"),
        }
    }
}
