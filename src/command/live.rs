use std::fs::PathExt;
use std::sync::mpsc::channel;
use std::thread;

use docopt::Docopt;
use tempdir::TempDir;
use time::{SteadyTime, Duration};
use notify::{RecommendedWatcher, Error, Watcher};
use iron::Iron;
use mount::Mount;
use staticfile::Static;

use command::Command;
use site::Site;
use configuration::Configuration;

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
    _temp_dir: TempDir,
    site: Site,
}

impl Live {
    pub fn new(mut configuration: Configuration) -> Live {
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

        configuration.is_preview = true;

        let temp_dir =
            TempDir::new(configuration.output.file_name().unwrap().to_str().unwrap())
                .unwrap();

        configuration.output = temp_dir.path().to_path_buf();

        println!("output dir: {:?}", configuration.output);

        Live {
            site: Site::new(configuration),
            _temp_dir: temp_dir,
        }
    }
}

impl Command for Live {
    fn site(&mut self) -> &mut Site {
        &mut self.site
    }

    fn run(&mut self) {
        let (e_tx, e_rx) = channel();

        let mut mount = Mount::new();
        mount.mount("/", Static::new(&self.site.configuration().output));

        let _guard = Iron::new(mount).http("0.0.0.0:3000").unwrap();

        let target = self.site.configuration().input.clone();

        thread::spawn(move || {
            let (tx, rx) = channel();
            let w: Result<RecommendedWatcher, Error> = Watcher::new(tx);

            match w {
                Ok(mut watcher) => {
                    match watcher.watch(&target) {
                        Ok(_) => {},
                        Err(_) => {
                            println!("some error with the live command");
                            ::std::process::exit(1);
                        },
                    }

                    for event in rx.iter() {
                        println!("got event for {:?}", event.path);

                        match event.op {
                            Ok(op) => {
                                match op {
                                    ::notify::op::CHMOD => trace!("Operation: chmod"),
                                    ::notify::op::CREATE => trace!("Operation: create"),
                                    ::notify::op::REMOVE => trace!("Operation: remove"),
                                    ::notify::op::RENAME => trace!("Operation: rename"),
                                    ::notify::op::WRITE => trace!("Operation: write"),
                                    _ => trace!("Operation: unknown"),
                                }
                            },
                            Err(e) => {
                                match e {
                                    ::notify::Error::Generic(e) => trace!("Error: {}", e),
                                    ::notify::Error::Io(e) => trace!("Error: {:?}", e),
                                    ::notify::Error::NotImplemented =>
                                        trace!("Error: Not Implemented"),
                                    ::notify::Error::PathNotFound =>
                                        trace!("Error: Path Not Found"),
                                    ::notify::Error::WatchNotFound =>
                                        trace!("Error: Watch Not Found"),
                                }
                                println!("notification error");
                                ::std::process::exit(1);
                            }
                        }

                        e_tx.send((event, SteadyTime::now()));
                    }
                },
                Err(_) => println!("Error"),
            }
        });

        self.site.build();

        let mut last_event = SteadyTime::now();
        let debounce = Duration::seconds(1);

        for (event, tm) in e_rx.iter() {
            let delta = tm - last_event;

            if delta < debounce {
                continue;
            }

            if let Some(ref pattern) = self.site.configuration().ignore {
                if event.path.as_ref().map(|p| pattern.matches(p)).unwrap_or(false) {
                    continue;
                }
            }

            if let Some(ref path) = event.path {
                while !path.exists() {
                    // TODO: this should probably be thread::yield_now
                    thread::park_timeout_ms(10);
                }
            }

            // TODO
            // this would probably become something like self.site.update();
            self.site.build();

            last_event = SteadyTime::now();
        }
    }
}
