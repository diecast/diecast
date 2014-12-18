use regex::Regex;

// perhaps routing should occur until after all
// of the compilers run but before the file is (possibly) written
// and it should take an Item so it could route based on metadata?
//
// e.g. to route to a folder named after the year the post was published

pub trait Route: Send + Sync {
  fn route(&self, from: &Path) -> Path;
}

// impl Route for &'static (Route + Send + Sync) {
//   fn route(&self, from: &Path) -> Path {
//     (**self).route(from)
//   }
// }

// impl Route for Box<Route + Send + Sync> {
//   fn route(&self, from: &Path) -> Path {
//     (**self).route(from)
//   }
// }

/// gen.route(Path::new("something.txt"))
impl Route for Path {
  fn route(&self, _from: &Path) -> Path {
    self.clone()
  }
}

impl<F> Route for F where F: Fn(&Path) -> Path, F: Send + Sync {
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
    identity(from)
  }
}

