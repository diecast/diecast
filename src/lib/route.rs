use regex::Regex;

// perhaps routing should occur until after all
// of the compilers run but before the file is (possibly) written
// and it should take an Item so it could route based on metadata?
//
// e.g. to route to a folder named after the year the post was published

pub trait Route {
  fn route(&self, from: &Path) -> Path;
}

/// gen.route(Path::new("something.txt"))
impl Route for Path {
  fn route(&self, _from: &Path) -> Path {
    self.clone()
  }
}

/// file.txt -> file.txt
/// gen.route(Identity)
pub struct Identity;

impl Route for Identity {
  fn route(&self, from: &Path) -> Path {
    from.clone()
  }
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
pub struct RegexRoute {
  regex: Regex,

  // perhaps use regex::Replacer instead?
  template: &'static str,
}

impl RegexRoute {
  pub fn new(regex: Regex, template: &'static str) -> RegexRoute {
    RegexRoute {
      regex: regex,
      template: template,
    }
  }
}

impl Route for RegexRoute {
  fn route(&self, from: &Path) -> Path {
    if let Some(path_str) = from.as_str() {
      if let Some(caps) = self.regex.captures(path_str) {
        return Path::new(caps.expand(self.template));
      }
    }

    // handle failure better
    Identity.route(from)
  }
}

