use std::collections::HashMap;
use std::error::Error;

use docopt::{self, Docopt};
use configuration::Configuration;
use rustc_serialize::{Decodable, Decoder};

use site::Site;
use rule::Rule;

pub mod build;
pub mod clean;

pub struct Plugin {
    name: String,
    description: String,
    constructor: fn(Site) -> Box<Command>,
}

impl Plugin {
    pub fn new<N, D>(
        name: N,
        description: D,
        constructor: fn(Site) -> Box<Command>
    ) -> Plugin
    where N: Into<String>, D: Into<String> {
        Plugin {
            name: name.into(),
            description: description.into(),
            constructor: constructor,
        }
    }
}

pub trait Command {
    fn run(&mut self) -> ::Result<()>;
}

impl<C> Command for Box<C>
where C: Command {
    fn run(&mut self) -> ::Result<()> {
        (**self).run()
    }
}

#[derive(RustcDecodable, Debug)]
struct Options {
    arg_command: Option<String>,
    arg_args: Vec<String>,
}

static USAGE: &'static str = "
Usage:
    diecast <command> [<args>...]
    diecast [options]

Options:
    -h, --help           Print this message
    -v, --version        Print version info

Possible commands include:
";

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

pub struct Builder {
    site: Site,
    plugins: HashMap<String, Plugin>,
}

impl Builder {
    pub fn new(site: Site) -> Builder {
        let mut plugins = HashMap::new();

        let build = build::plugin();
        let clean = clean::plugin();

        plugins.insert(build.name.clone(), build);
        plugins.insert(clean.name.clone(), clean);

        Builder {
            site: site,
            plugins: plugins,
        }
    }

    pub fn plugin(mut self, plugin: Plugin) -> Builder {
        self.plugins.insert(plugin.name.clone(), plugin);
        self
    }

    pub fn build(mut self) -> Result<Box<Command>, Box<Error>> {
        let mut usage = String::from(USAGE);

        let mut plugs =
            self.plugins.iter()
            .collect::<Vec<(&String, &Plugin)>>();

        plugs.sort_by(|a, b| a.0.cmp(b.0));

        for &(k, v) in &plugs {
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

        let options: Options =
            try! {
                Docopt::new(usage.clone())
                .and_then(|d|
                    d
                    .options_first(true)
                    .help(true)
                    .version(Some(version()))
                    .decode())
            };

        let cmd = options.arg_command.unwrap();
        self.site.configuration_mut().command = cmd.clone();

        let err =
            Err(From::from(docopt::Error::WithProgramUsage(
                Box::new(docopt::Error::Help),
                String::from(usage))));

        let command: Box<Command> = match &cmd[..] {
            "" | "help" => return err,
            cmd => {
                if let Some(plugin) = self.plugins.get(cmd) {
                    (plugin.constructor)(self.site)
                } else {
                    // here look in PATH to find program named diecast-$cmd
                    // if not found, then output this message:
                    println!("unknown command `{}`", cmd);
                    return err;
                }
            },
        };

        Ok(command)
    }
}
