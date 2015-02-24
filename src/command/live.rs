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
            TempDir::new(
                configuration.output.file_name().unwrap()
                    .to_str().unwrap()).unwrap();

        configuration.output = PathBuf::new(temp_dir.path().as_str().unwrap());

        println!("output dir: {:?}", configuration.output);

        Live {
            site: Site::new(configuration),
            temp_dir: temp_dir,
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

        // TODO: once iron gets fixed, use that instead
        Server::new("python2")
            .arg("-m")
            .arg("SimpleHTTPServer")
            .arg("3000")
            .current_dir(&self.site.configuration().output)
            .spawn();

        // let mut mount = Mount::new();
        // mount.mount(
        //     "/",
        //     Static::new(configuration.output.file_name().unwrap().to_str().unwrap()).unwrap());

        // Iron::new(mount).listen((Ipv4Addr(127, 0, 0, 1), 3000)).unwrap();

        match w {
            Ok(mut watcher) => {
                watcher.watch(&self.site.configuration().input);

                self.site.build();

                for _event in rx.iter() {
                    self.site.build();
                }
            },
            Err(_) => println!("Error"),
        }
    }
}
