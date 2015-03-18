use item::{self, Item};
use std::path::PathBuf;
use compiler;

use regex;

// perhaps routing should occur until after all
// of the compilers run but before the file is (possibly) written
// and it should take an Item so it could route based on metadata?
//
// e.g. to route to a folder named after the year the post was published

/// file.txt -> file.txt
/// gen.route(Identity)
pub fn identity(item: &mut Item) {
    trace!("routing {} with the identity router", item.from.clone().unwrap().display());
    item.to = item.from.clone();
}

pub fn set_extension(extension: &'static str) -> Box<item::Handler + Sync + Send> {
    Box::new(move |item: &mut Item| -> compiler::Result {
        if let Some(ref from) = item.from {
            item.to = Some(from.with_extension(extension));
        }

        Ok(())
    })
}

/// file.txt -> file.html
/// gen.route(SetExtension::new("html"))
#[derive(Copy, Clone)]
pub struct SetExtension {
    extension: &'static str,
}

impl SetExtension {
    pub fn new(extension: &'static str) -> SetExtension {
        SetExtension {
            extension: extension,
        }
    }
}

impl item::Handler for SetExtension {
    fn handle(&self, item: &mut Item) -> compiler::Result {
        let mut cloned = item.from.clone().unwrap();
        cloned.set_extension(self.extension);
        item.to = Some(cloned);
        Ok(())
    }
}

/// regex expansion
///
/// gen.route(
///     RegexRoute::new(
///         regex!("/posts/post-(?P<name>.+)\.markdown"),
///         "/target/$name.html"));
#[derive(Clone)]
pub struct Regex {
    regex: regex::Regex,

    // perhaps use regex::Replacer instead?
    // http://doc.rust-lang.org/regex/regex/trait.Replacer.html
    template: &'static str,
}

impl Regex {
    pub fn new(regex: regex::Regex, template: &'static str) -> Regex {
        Regex {
            regex: regex,
            template: template,
        }
    }
}

impl item::Handler for Regex {
    fn handle(&self, item: &mut Item) -> compiler::Result {
        let from = item.from.clone().unwrap();
        let path_str = from.to_str().unwrap();

        if let Some(caps) = self.regex.captures(path_str) {
            item.to = Some(PathBuf::from(&caps.expand(self.template)));
        }

        Ok(())
    }
}

