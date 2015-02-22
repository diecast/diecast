use docopt::Docopt;
use site::Site;
use configuration::Configuration;

use std::old_io::TempDir;
use std::path::PathBuf;
use std::process::Command as Server;

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

        configuration.is_preview = true;

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

        Server::new("python2")
            .arg("-m")
            .arg("SimpleHTTPServer")
            .arg("3000")
            .current_dir(&site.configuration().output)
            .spawn();

        // let mut mount = Mount::new();
        // mount.mount(
        //     "/",
        //     Static::new(configuration.output.file_name().unwrap().to_str().unwrap()).unwrap());

        // Iron::new(mount).listen((Ipv4Addr(127, 0, 0, 1), 3000)).unwrap();

        match w {
            Ok(mut watcher) => {
                watcher.watch(&site.configuration().input);

                site.build();

                for event in rx.iter() {
                    site.build();
                }
            },
            Err(e) => println!("Error"),
        }
    }
}
