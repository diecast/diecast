use std::collections::HashMap;

use docopt::{self, Docopt};
use configuration::Configuration;
use rustc_serialize::{Decodable, Decoder};

use site::Site;
use rule::Rule;

pub mod build;
pub mod clean;
pub mod live;

pub struct Plugin {
    name: String,
    description: String,
    constructor: fn(Vec<Rule>, Configuration) -> Box<Command>,
}

pub trait Command {
    fn run(&mut self);
}

impl<C> Command for Box<C>
where C: Command {
    fn run(&mut self) {
        (**self).run();
    }
}

#[derive(RustcDecodable, Debug)]
struct Options {
    arg_command: Option<String>,
    arg_args: Vec<String>,
}

pub fn version() -> String {
    format!("diecast {}", match option_env!("CFG_VERSION") {
        Some(s) => String::from(s),
        Option::None => format!("{}.{}.{}{}",
                        env!("CARGO_PKG_VERSION_MAJOR"),
                        env!("CARGO_PKG_VERSION_MINOR"),
                        env!("CARGO_PKG_VERSION_PATCH"),
                        option_env!("CARGO_PKG_VERSION_PRE").unwrap_or(""))
    })
}

pub fn from_args(rules: Vec<Rule>, mut configuration: Configuration) -> Box<Command> {
    let mut usage: String = String::from("
Usage:
    diecast <command> [<args>...]
    diecast [options]

Options:
    -h, --help           Print this message
    -v, --version        Print version info

Possible commands include:
");

    let mut plugins: Vec<Plugin> = vec![];

    // TODO
    // * don't require String
    // * don't require separate constructor fn
    plugins.push(Plugin {
        name: String::from("build"),
        description: String::from("Build site"),
        constructor: build::Build::plugin,
    });

    plugins.push(Plugin {
        name: String::from("clean"),
        description: String::from("Remove output directory"),
        constructor: clean::Clean::plugin,
    });

    plugins.push(Plugin {
        name: String::from("live"),
        description: String::from("Preview the site live"),
        constructor: live::Live::plugin,
    });

    let mut commands: HashMap<String, Plugin> = HashMap::new();

    // later plugins override older ones
    for plugin in plugins {
        commands.insert(plugin.name.clone(), plugin);
    }

    // information needed:
    // * constructor
    // * name
    // * description

    // * build       Build site
    // host        Host a server
    // preview     Build site in preview mode and host preview web server
    // watch       Watch files and re-build when a file changes

    // iterate here so that we only use the actually-registered commands
    for (k, v) in &commands {
        usage.push_str("    ");
        usage.push_str(&k);

        // TODO: proper padding
        if k.len() > 11 {
            panic!("the command name should be less than 12 characters");
        }

        let pad = 12 - k.len();
        usage.push_str(&::std::iter::repeat(' ').take(pad).collect::<String>());
        usage.push_str(&v.description);
        usage.push('\n');
    }

    let docopt =
        Docopt::new(usage.clone())
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
            usage)
            .exit();
    }

    let cmd = options.arg_command.unwrap();
    configuration.command = cmd.clone();

    let command: Box<Command> = match &cmd[..] {
        "help" => {
            docopt::Error::WithProgramUsage(
                Box::new(docopt::Error::Help),
                String::from(usage))
                .exit();
        },
        // "build" => Box::new(build::Build::new(configuration)),
        // "live" => Box::new(live::Live::new(configuration)),
        // "clean" => Box::new(clean::Clean::new(configuration)),
        "" => {
            unreachable!();
        },
        cmd => {
            if let Some(plugin) = commands.get(cmd) {
                (plugin.constructor)(rules, configuration)
            } else {
                // here look in PATH to find program named diecast-$cmd
                // if not found, then output this message:
                println!("unknown command `{}`", cmd);
                docopt::Error::WithProgramUsage(
                    Box::new(docopt::Error::Help),
                    String::from(usage))
                    .exit();
            }
        },
    };

    command
}
