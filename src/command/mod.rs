use docopt::{self, Docopt};
use configuration::Configuration;
use rustc_serialize::{Decodable, Decoder};

use self::Kind::*;

use site::Site;

pub mod build;
pub mod clean;
pub mod live;

pub trait Command {
    fn site(&mut self) -> &mut Site;
    fn run(&mut self);
}

impl<C: ?Sized> Command for Box<C> where C: Command {
    fn run(&mut self) {
        (**self).run();
    }

    fn site(&mut self) -> &mut Site {
        (**self).site()
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
    arg_command: Option<Kind>,
    arg_args: Vec<String>,
}

#[derive(Debug)]
pub enum Kind {
    Build,
    Live,
    Clean,
    Help,
    Other(String),
    None,
}

impl Decodable for Kind {
    fn decode<D: Decoder>(d: &mut D) -> Result<Kind, D::Error> {
        use self::Kind::*;

        let s = try!(d.read_str());

        let command = match &s[..] {
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
        Option::None => format!("{}.{}.{}{}",
                        env!("CARGO_PKG_VERSION_MAJOR"),
                        env!("CARGO_PKG_VERSION_MINOR"),
                        env!("CARGO_PKG_VERSION_PATCH"),
                        option_env!("CARGO_PKG_VERSION_PRE").unwrap_or(""))
    })
}

pub fn from_args(mut configuration: Configuration) -> Box<Command> {
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

    configuration.command = options.arg_command.unwrap();

    let command: Box<Command> = match configuration.command {
        Build => Box::new(build::Build::new(configuration)),
        Live => Box::new(live::Live::new(configuration)),
        Clean => Box::new(clean::Clean::new(configuration)),
        Help => {
            docopt::Error::WithProgramUsage(
                Box::new(docopt::Error::Help),
                USAGE.to_string())
                .exit();
        },
        Other(ref cmd) => {
            // TODO:
            //
            // here check if cmd is a registered command?
            //
            // approach:
            // * return an enum? Command(Box<Command>) or Custom(name)

            // here look in PATH to find program named diecast-$cmd
            // if not found, then output this message:
            println!("unknown command `{}`", cmd);
            docopt::Error::WithProgramUsage(
                Box::new(docopt::Error::Help),
                USAGE.to_string())
                .exit();
        },
        None => {
            panic!("can't create a command from `None`");
        }
    };

    command
}
