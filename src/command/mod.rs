use std::collections::HashMap;
use std::error::Error;

use docopt::{self, Docopt};

use site::Site;

pub mod build;
pub mod clean;
pub mod deploy;

pub trait Command {
    // TODO
    // not sure that it should have a description method
    // this should probably be provided separately?
    fn description(&self) -> &'static str;
    fn run(&mut self, site: &mut Site) -> ::Result<()>;
}

impl<C> Command for Box<C>
where C: Command {
    fn description(&self) -> &'static str {
        (**self).description()
    }

    fn run(&mut self, site: &mut Site) -> ::Result<()> {
        (**self).run(site)
    }
}

#[derive(Deserialize, Debug)]
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
    commands: HashMap<String, Box<Command>>,
}

impl Builder {
    pub fn new() -> Builder {
        let builder = Builder {
            commands: HashMap::new(),
        };

        builder
            .command("build", build::Build)
            .command("clean", clean::Clean)
    }

    pub fn command<S, C>(mut self, name: S, command: C) -> Builder
    where S: Into<String>, C: Command + 'static {
        self.commands.insert(name.into(), Box::new(command));
        self
    }

    pub fn build(mut self) -> Result<Box<Command>, Box<Error>> {
        let mut usage = String::from(USAGE);

        {
            let mut cmds =
                self.commands.iter()
                .collect::<Vec<(&String, &Box<Command>)>>();

            cmds.sort_by(|a, b| a.0.cmp(b.0));

            for &(k, v) in &cmds {
                usage.push_str("    ");
                usage.push_str(&k);

                // TODO: proper padding
                if k.len() > 11 {
                    panic!("the command name should be less than 12 characters");
                }

                let pad = 12 - k.len();
                usage.push_str(&::std::iter::repeat(' ').take(pad).collect::<String>());
                usage.push_str(&v.description());
                usage.push('\n');
            }
        }

        let options: Options =
            Docopt::new(usage.clone())
                .and_then(|d|
                    d
                    .options_first(true)
                    .help(true)
                    .version(Some(version()))
                    .deserialize())?;

        let cmd = options.arg_command.unwrap();

        let err =
            Err(From::from(docopt::Error::WithProgramUsage(
                Box::new(docopt::Error::Help),
                String::from(usage))));

        // TODO
        // the way this was changed prevents any commands from being passed
        // to the sub-command. what needs to happen is that the flags need to
        // be passed onto the sub-command once it is chosen
        //
        // Use Docopt.argv() to explicitly pass the _full_ arguments including
        // the binary name.
        //
        // construct these args here, same as cargo is doing here:
        // https://github.com/rust-lang/cargo/blob/master/src/bin/cargo.rs#L141
        //
        // UPDATE
        // false alarm? The command runs Docopt itself on env::args() so
        // everything seems to be working fine, it's just that the command has
        // to ensure that Docopt is run and its effects take place
        //
        // that said, perhaps it _is_ beneficial to explicitly pass the argv to
        // the command, so that the root command can support `diecast help subcommand`,
        // which ends up rewriting the argv to [diecast, subcommand, -h]
        //
        // that may also be necessary to support external diecast-cmd binaries
        // in PATH?
        let command: Box<Command> = match &cmd[..] {
            "" | "help" if options.arg_args.is_empty() => return err,
            cmd => {
                if let Some(command) = self.commands.remove(cmd) {
                    command
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
