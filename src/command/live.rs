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
        let (tx, rx) = channel();
        let w: Result<RecommendedWatcher, Error> = Watcher::new(tx);

        let mut mount = Mount::new();
        mount.mount("/", Static::new(&self.site.configuration().output));

        let _guard = Iron::new(mount).http("0.0.0.0:3000").unwrap();

        let mut last_event = SteadyTime::now();
        let debounce = Duration::seconds(1);

        match w {
            Ok(mut watcher) => {
                match watcher.watch(&self.site.configuration().input) {
                    Ok(_) => {},
                    Err(_) => {
                        println!("some error with the live command");
                        ::std::process::exit(1);
                    },
                }

                self.site.build();

                for event in rx.iter() {
                    let current_time = SteadyTime::now();
                    let delta = current_time - last_event;

                    trace!("got event for {:?}", event.path);
                    trace!("delta is {}", delta);

                    if let Some(ref pattern) = self.site.configuration().ignore {
                        if event.path.as_ref().map(|p| pattern.matches(p)).unwrap_or(false) {
                            trace!("is ignored file; skipping");
                            continue;
                        }
                    }

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

                    if delta < debounce {
                        trace!("within debounce span; skipping");
                        continue;
                    }

                    trace!("new event; setting new time ({} → {})", last_event, current_time);
                    last_event = current_time;

                    if let Some(ref path) = event.path {
                        while !path.exists() {
                            // TODO: this should probably be thread::yield_now
                            thread::park_timeout_ms(10);
                        }
                    }

                    // TODO
                    // this would probably become something like self.site.update();
                    self.site.build();
                }
            },
            Err(_) => println!("Error"),
        }
    }
}
