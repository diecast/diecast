use item::{Item, Dependencies};
use compiler::Compile;

use regex;

// perhaps routing should occur until after all
// of the compilers run but before the file is (possibly) written
// and it should take an Item so it could route based on metadata?
//
// e.g. to route to a folder named after the year the post was published

/// file.txt -> file.txt
/// gen.route(Identity)
pub fn identity(item: &mut Item, _deps: Option<Dependencies>) {
    println!("routing {} with the identity router", item.from.clone().unwrap().display());
    item.to = item.from.clone();
}

/// file.txt -> file.html
/// gen.route(SetExtension::new("html"))
#[derive(Copy)]
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

impl Compile for SetExtension {
    fn compile(&self, item: &mut Item, _deps: Option<Dependencies>) {
        let mut cloned = item.from.clone().unwrap();
        cloned.set_extension(self.extension);
        item.to = Some(cloned);
    }
}

/// regex expansion
///
/// gen.route(
///     RegexRoute::new(
///         regex!("/posts/post-(?P<name>.+)\.markdown"),
///         "/target/$name.html"));
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

impl Compile for Regex {
    fn compile(&self, item: &mut Item, _deps: Option<Dependencies>) {
        let from = item.from.clone().unwrap();
        let path_str = from.as_str().unwrap();

        if let Some(caps) = self.regex.captures(path_str) {
            item.to = Some(Path::new(caps.expand(self.template)));
        }
    }
}

