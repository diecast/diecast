pub trait Router {
  fn route(&self, &Path) -> Path;
}

///! generator.router((regex!(r"src/something/(.+)\.markdown"), "$1/index.html"))

// impl Router for (Regex, Replacer) {
//   fn route(&self, p: &Path) -> Path {
//     self.0.replace(p.filename_str().unwrap(), self.1)
//   }
// }

///! generator.router(identity)

impl Router for fn(&Path) -> Path {
  fn route(&self, p: &Path) -> Path {
    (*self)(p)
  }
}

///! generator.router(|p: &Path| Path::new("hi.txt"))

impl<'a> Router for |&Path|: 'a -> Path {
  fn route(&self, p: &Path) -> Path {
    (*self)(p)
  }
}

// this requires higher-rank lifetimes
// https://github.com/rust-lang/rust/issues/15067
// https://github.com/rust-lang/rfcs/blob/master/active/0044-closures.md
// impl<'a, F> Router for F
//   where F: FnMut(&'a Path) -> Path {
//   fn route(&self, p: &Path) -> Path {
//     // self(p)
//     self.call_mut((p,))
//   }
// }

pub fn identity(p: &Path) -> Path {
  Path::new(p.filename_str().unwrap())
}

