use docopt::{self, Docopt};
use site::{Rule, Configuration};
use std::str::FromStr;
use std::env;
use rustc_serialize;

use self::Command::*;

use commands;

static USAGE: &'static str = "
Usage:
    diecast <command> [<args>...]
    diecast [options]

Options:
    -h, --help           Print this message
    -V, --version        Print version info
    -v, --verbose        Use verbose output

Possible commands include:
    build       Build site
    clean       Remove output directory
    preview     Build site in preview mode and host preview web server
    watch       Watch files and re-build when a file changes
";

#[derive(RustcDecodable, Debug)]
struct Options {
    arg_command: String,
    arg_args: Vec<String>,
    flag_verbose: bool,
}

pub struct Diecast {
    configuration: Configuration,
    rules: Vec<Rule>,
}

#[derive(Debug)]
pub enum Command {
    Build,
    Clean,
    Help,
}

impl FromStr for Command {
    type Err = String;

    fn from_str(s: &str) -> Result<Command, String> {
        use std::ascii::AsciiExt;
        use self::Command::*;

        let lower = s.to_ascii_lowercase();

        let command = match &lower[] {
            "build" => Build,
            "clean" => Clean,
            "help"  => Help,
            _ => return Err(format!("no such command: `{}`", lower).to_string()),
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

impl Diecast {
    pub fn new(configuration: Configuration) -> Diecast {
        Diecast {
            configuration: configuration,
            rules: vec![],
        }
    }

    pub fn rule(mut self, rule: Rule) -> Diecast {
        self.rules.push(rule);
        self
    }

    pub fn run(self) {
        for arg in ::std::env::args() {
            print!("{:?} ", arg);
        }

        println!("");

        let docopt =
            Docopt::new(USAGE)
                .unwrap_or_else(|e| e.exit())
                .options_first(true)
                .help(true)
                .version(Some(version()));

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

        if options.arg_command.is_empty() {
            println!("no command present");
            help_error.exit();
        }

        let command: Command =
            match options.arg_command.parse() {
                Ok(cmd) => cmd,
                Err(e) => {
                    println!("\n{}", e);
                    help_error.exit();
                }
            };

        println!("{:?}", command);

        match command {
            Build => commands::build::execute(),
            Help => help_error.exit(),
            Clean => {
                // remove output dir
            },
        }
    }
}
