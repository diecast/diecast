use item::Item;
use handle::Handle;
use std::path::{PathBuf, Path};

use regex;

// perhaps routing should occur until after all
// of the handlers run but before the file is (possibly) written
// and it should take an Item so it could route based on metadata?
//
// e.g. to route to a folder named after the year the post was published

/// file.txt -> file.txt
/// gen.route(Identity)
pub fn identity(item: &mut Item) -> ::Result {
    item.route_with(|path: &Path| -> PathBuf {
        path.to_path_buf()
    });

    Ok(())
}

pub fn pretty(item: &mut Item) -> ::Result {
    item.route_with(|path: &Path| -> PathBuf {
        let mut result = path.with_extension("");
        result.push("index.html");
        result
    });

    Ok(())
}

// TODO fallback semantics
// currently if there is no file_name, then keeps same path?
pub fn pretty_page(item: &mut Item) -> ::Result {
    item.route_with(|path: &Path| -> PathBuf {
        let without = path.with_extension("");

        if let Some(file_name) = without.file_name() {
            let mut result = PathBuf::from(file_name);
            result.push("index.html");
            result
        } else {
            path.to_path_buf()
        }
    });

    Ok(())
}

#[inline]
pub fn set_extension(extension: &'static str) -> SetExtension {
    SetExtension {
        extension: extension,
    }
}

/// file.txt -> file.html
/// gen.route(SetExtension::new("html"))
#[derive(Copy, Clone)]
pub struct SetExtension {
    extension: &'static str,
}

impl Handle<Item> for SetExtension {
    fn handle(&self, item: &mut Item) -> ::Result {
        item.route_with(|path: &Path| -> PathBuf {
            path.with_extension(self.extension)
        });

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

impl Handle<Item> for Regex {
    fn handle(&self, item: &mut Item) -> ::Result {
        item.route_with(|path: &Path| -> PathBuf {
            let caps = self.regex.captures(path.to_str().unwrap()).unwrap();
            PathBuf::from(&caps.expand(self.template))
        });

        Ok(())
    }
}

