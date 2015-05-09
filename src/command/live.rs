use std::fs::PathExt;
use std::sync::mpsc::{channel, TryRecvError};
use std::path::PathBuf;
use std::collections::HashSet;
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

                    // TODO
                    // what if the user saves the buffer,
                    // notices a typo within 1-2 seconds, fixes it,
                    // then saves again?
                    //
                    // the second save will unlikely occur within the rebounce
                    // period, and will probably end up being debounced since
                    // it occurred while the site was building due to the first
                    // save
                    //
                    // I think this behavior is understandable. the user could
                    // tune the rebounce period to be higher, e.g. 5 seconds,
                    // to be able to catch these kinds of quick fixes, at the expense
                    // of lack of immediacy
                    let rebounce = Duration::milliseconds(300);
                    let mut last_bounce = SteadyTime::now();
                    let mut set: HashSet<PathBuf> = HashSet::new();

                    loop {
                        match rx.try_recv() {
                            Ok(event) => {
                                trace!(">>> received {:?}", event.path);

                                let now = SteadyTime::now();
                                let is_contained = event.path.as_ref().map(|p| set.contains(p)).unwrap_or(false);

                                // TODO: check time despite event presence

                                // past rebounce period
                                if (now - last_bounce) > rebounce {
                                    trace!(">>> past rebounce period");

                                    if !set.is_empty() {
                                        trace!(">>> sending events");
                                        e_tx.send((set, now)).unwrap();
                                        set = HashSet::new();
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

                                trace!(">>> within rebounce period");

                                if let Some(path) = event.path {
                                    if !is_contained {
                                        last_bounce = now;
                                        trace!(">>> extending rebounce");

                                        trace!(">>> inserting path");
                                        set.insert(path);
                                    } else {
                                        trace!(">>> already contained path");
                                    }
                                }
                            },
                            Err(TryRecvError::Empty) => {
                                let now = SteadyTime::now();

                                if (now - last_bounce) > rebounce {
                                    last_bounce = now;

                                    if !set.is_empty() {
                                        trace!(">>> sending events");
                                        e_tx.send((set, now)).unwrap();
                                        set = HashSet::new();
                                    }

                                    continue;
                                } else {
                                    // TODO audit
                                    // consume rebounce time in 100ms chunks
                                    thread::sleep_ms(100);
                                }
                            },
                            Err(TryRecvError::Disconnected) => {
                                panic!("notification manager disconnected");
                            },
                        }
                    }
                },
                Err(_) => println!("Error"),
            }
        });

        self.site.build();
        println!("finished building");

        let mut last_event = SteadyTime::now();
        let debounce = Duration::seconds(1);

        for (mut paths, tm) in e_rx.iter() {
            trace!("received paths:\n{:?}", paths);

            let delta = tm - last_event;

            if delta < debounce {
                trace!("debounced");
                continue;
            }

            if let Some(ref pattern) = self.site.configuration().ignore {
                trace!("filtering");

                paths = paths.into_iter()
                    .filter(|p| !pattern.matches(p))
                    .collect::<HashSet<PathBuf>>();
            }

            trace!("paths:\n{:?}", paths);

            let (mut ready, mut waiting): (HashSet<PathBuf>, HashSet<PathBuf>) =
                paths.into_iter().partition(|p| p.exists());

            // TODO optimize
            // so only non-existing paths are still polled?
            // perhaps using a partition
            while !waiting.is_empty() {
                trace!("waiting for all paths to exist");
                // TODO: this should probably be thread::yield_now
                thread::park_timeout_ms(10);

                let (r, w): (HashSet<PathBuf>, HashSet<PathBuf>) =
                    waiting.into_iter().partition(|p| p.exists());

                ready.extend(r.into_iter());
                waiting = w;
            }

            paths = ready;

            trace!("updating");

            // TODO
            // this would probably become something like self.site.update();
            let p = paths.into_iter()
            .map(|p| p.relative_from(&self.site.configuration().input).unwrap().to_path_buf())
            .collect::<HashSet<PathBuf>>();
            println!("mapped: {:?}", p);
            self.site.update(p);

            trace!("finished updating");

            last_event = SteadyTime::now();
        }

        panic!("exited live loop");
    }
}
