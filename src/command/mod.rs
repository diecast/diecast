use docopt::{self, Docopt};
use rule::Rule;
use configuration::Configuration;
use std::str::FromStr;
use std::env;
use rustc_serialize::{Decodable, Decoder};
use std::sync::Arc;

use self::CommandKind::*;

use site::Site;

pub mod build;
pub mod clean;
pub mod live;

pub trait Command {
    fn run(&self, site: Site);
}

impl<C: ?Sized> Command for Box<C> where C: Command {
    fn run(&self, site: Site) {
        (**self).run(site);
    }
}

static USAGE: &'static str = "
Usage:
    diecast <command> [<args>...]
    diecast [options]

Options:
    -h, --help           Print this message
    -v, --version        Print version info

Possible commands include:
    build       Build site
    host        Host a server
    live        Preview the site live
    preview     Build site in preview mode and host preview web server
    clean       Remove output directory
    watch       Watch files and re-build when a file changes
";

#[derive(RustcDecodable, Debug)]
struct Options {
    arg_command: Option<CommandKind>,
    arg_args: Vec<String>,
}

#[derive(Debug)]
pub enum CommandKind {
    Build,
    Live,
    Clean,
    Help,
    Other(String),
}

impl Decodable for CommandKind {
    fn decode<D: Decoder>(d: &mut D) -> Result<CommandKind, D::Error> {
        use std::ascii::AsciiExt;
        use self::CommandKind::*;

        let s = try!(d.read_str());

        let command = match &s[] {
            "build" => Build,
            "live" => Live,
            "clean" => Clean,
            "help"  => Help,
            s => Other(s.to_string()),
        };

        Ok(command)
    }
}

pub fn version() -> String {
    format!("diecast {}", match option_env!("CFG_VERSION") {
        Some(s) => s.to_string(),
        None => format!("{}.{}.{}{}",
                        env!("CARGO_PKG_VERSION_MAJOR"),
                        env!("CARGO_PKG_VERSION_MINOR"),
                        env!("CARGO_PKG_VERSION_PATCH"),
                        option_env!("CARGO_PKG_VERSION_PRE").unwrap_or(""))
    })
}

pub fn from_args(mut configuration: Configuration) -> (Box<Command>, Site) {
    let docopt =
        Docopt::new(USAGE)
            .unwrap_or_else(|e| e.exit())
            .options_first(true)
            .help(true)
            .version(Some(version()));

    let options: Options = docopt.decode().unwrap_or_else(|e| {
        e.exit();
    });

    if options.arg_command.is_none() {
        docopt::Error::WithProgramUsage(
            Box::new(docopt::Error::Help),
            USAGE.to_string())
            .exit();
    }

    let command = match options.arg_command.unwrap() {
        Build => Box::new(build::Build::new(&mut configuration)) as Box<Command>,
        Live => Box::new(live::Live::new(&mut configuration)) as Box<Command>,
        Clean => Box::new(clean::Clean::new(&mut configuration)) as Box<Command>,
        Help => {
            docopt::Error::WithProgramUsage(
                Box::new(docopt::Error::Help),
                USAGE.to_string())
                .exit();
        },
        Other(cmd) => {
            // here look in PATH to find program named diecast-$cmd
            // if not found, then output this message:
            println!("unknown command `{}`", cmd);
            docopt::Error::WithProgramUsage(
                Box::new(docopt::Error::Help),
                USAGE.to_string())
                .exit();
        },
    };

    (command, Site::new(configuration))
}
