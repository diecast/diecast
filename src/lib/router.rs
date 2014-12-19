use regex;

// perhaps routing should occur until after all
// of the compilers run but before the file is (possibly) written
// and it should take an Item so it could route based on metadata?
//
// e.g. to route to a folder named after the year the post was published

pub trait Route {
  fn route(&self, from: &Path) -> Path;
}

impl<'a, R> Route for &'a R where R: Route {
  fn route(&self, from: &Path) -> Path {
    (*self).route(from)
  }
}

impl<F> Route for F where F: Fn(&Path) -> Path {
  fn route(&self, from: &Path) -> Path {
    (*self)(from)
  }
}

/// file.txt -> file.txt
/// gen.route(Identity)
pub fn identity(from: &Path) -> Path {
  println!("routing {} with the identity router", from.display());
  from.clone()
}

/// file.txt -> file.html
/// gen.route(SetExtension::new("html"))
pub struct SetExtension {
  extension: String,
}

impl SetExtension {
  pub fn new(extension: String) -> SetExtension {
    SetExtension {
      extension: extension,
    }
  }
}

impl Route for SetExtension {
  fn route(&self, from: &Path) -> Path {
    let mut cloned = from.clone();
    cloned.set_extension(self.extension.as_slice());
    return cloned;
  }
}

/// regex expansion
///
/// gen.route(
///   RegexRoute::new(
///     regex!("/posts/post-(?P<name>.+)\.markdown"),
///     "/target/$name.html"));
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

impl Route for Regex {
  fn route(&self, from: &Path) -> Path {
    let path_str = from.as_str().unwrap();

    if let Some(caps) = self.regex.captures(path_str) {
      return Path::new(caps.expand(self.template));
    }

    identity(from)
  }
}

