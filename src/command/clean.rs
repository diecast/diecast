use docopt::Docopt;

use command::Command;
use configuration::Configuration;
use site::Site;

#[derive(Deserialize, Debug)]
struct Options {
    flag_verbose: bool,
    flag_ignore_hidden: bool,
}

// TODO
// the help message includes the wrong command
// e.g. if someone did:
//
//     .command("mess", clean)
//
// the `diecast help mess` will still show the `clean` command
static USAGE: &'static str = "
Usage:
    diecast clean [options]

Options:
    -h, --help            Print this message
    -v, --verbose         Use verbose output
    -i, --ignore-hidden   Don't clean out hidden files and directories

This removes the output directory.
";

pub struct Clean;

impl Clean {
    pub fn configure(&mut self, configuration: &mut Configuration) {
        let options: Options = Docopt::new(USAGE)
            .and_then(|d| d.help(true).deserialize())
            .unwrap_or_else(|e| e.exit());

        configuration.ignore_hidden = options.flag_ignore_hidden;
    }
}

impl Command for Clean {
    fn description(&self) -> &'static str {
        "Remove output directory"
    }

    fn run(&mut self, site: &mut Site) -> ::Result<()> {
        self.configure(site.configuration_mut());

        let target = &site.configuration().output;

        if target.exists() {
            println!("removing {:?}", target);
        } else {
            println!("nothing to remove");
        }

        // TODO: clean return Result?
        site.clean()
    }
}
