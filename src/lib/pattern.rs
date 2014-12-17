//! Pattern matching behavior.
//!
//! The DSL submodule exposes helper functions for
//! constructing instances of the various built-in
//! pattern types.
//!
//! The `pattern!` macro makes it easier to construct
//! these patterns by doing away with the need to manually
//! bring the dsl items into scope.
//!
//! ```ignore
//! let pat =
//!   or!(
//!     "posts/**",
//!     and!(
//!       "pages/**",
//!       not!("pages/secret-work.md")))
//! );
//! ```

use glob;
use regex::Regex;

/// A kind of pattern that can be used for
/// filtering the files in the input directory.
pub trait Pattern {
  fn matches(&self, &Path) -> bool;
}

/// The negation of a pattern.
pub struct NotPattern<P>
  where P: Pattern {
  pattern: P
}

impl<P> Pattern for NotPattern<P>
  where P: Pattern {
  fn matches(&self, p: &Path) -> bool {
    !self.pattern.matches(p)
  }
}

/// This conjunction of two patterns.
pub struct AndPattern<P1, P2>
  where P1: Pattern, P2: Pattern {
  left: P1,
  right: P2
}

impl<P1, P2> Pattern for AndPattern<P1, P2>
  where P1: Pattern, P2: Pattern {
  fn matches(&self, p: &Path) -> bool {
    self.left.matches(p) && self.right.matches(p)
  }
}

/// The disjunction of two patterns.
pub struct OrPattern<P1, P2>
  where P1: Pattern, P2: Pattern {
  left: P1,
  right: P2
}

impl<P1, P2> Pattern for OrPattern<P1, P2>
  where P1: Pattern, P2: Pattern {
  fn matches(&self, p: &Path) -> bool {
    self.left.matches(p) || self.right.matches(p)
  }
}

/// Pattern that matches everything.
#[deriving(Copy)]
pub struct Everything;

impl Pattern for Everything {
  fn matches(&self, _: &Path) -> bool {
    true
  }
}

/// Allow regular expression patterns.
impl Pattern for Regex {
  fn matches(&self, p: &Path) -> bool {
    self.is_match(p.as_str().unwrap())
  }
}

// TODO: consider making &str impl be exact match?
/// Treat string slices as globs.
///
/// This simply converts the string to glob::Pattern.
/// It's much more efficient to just use the glob::Pattern
/// to begin with.
impl<'a> Pattern for &'a str {
  fn matches(&self, p: &Path) -> bool {
    *self == p.as_str().unwrap()
  }
}

impl Pattern for glob::Pattern {
  fn matches(&self, p: &Path) -> bool {
    self.matches_path(p)
  }
}

/// Contains the DSL items for easily constructing complex patterns.
pub mod dsl {
  use super::{Pattern, NotPattern, AndPattern, OrPattern};

  /// Constructs the negation of a pattern.
  pub fn not<P>(p: P) -> NotPattern<P>
    where P: Pattern {
    NotPattern {
      pattern: p
    }
  }

  /// Constructs the conjunction of two patterns.
  pub fn and<P1, P2>(p1: P1, p2: P2) -> AndPattern<P1, P2>
    where P1: Pattern, P2: Pattern {
    AndPattern {
      left: p1,
      right: p2
    }
  }

  /// Constructs the disjunction of two patterns.
  pub fn or<P1, P2>(p1: P1, p2: P2) -> OrPattern<P1, P2>
    where P1: Pattern, P2: Pattern {
    OrPattern {
      left: p1,
      right: p2
    }
  }
}

#[cfg(test)]
mod test {
  use super::{Pattern, Everything};

  fn matches<T>(pattern: T, p: &Path) -> bool
    where T: Pattern {
    pattern.matches(p)
  }

  #[test]
  fn match_everything() {
    let intro_to_rust = Path::new("posts/long/introduction-to-rust.md");

    assert!(pattern!(Everything).matches(&intro_to_rust));
  }

  #[test]
  fn match_globs() {
    let posts = "posts/**.md";
    let intro_to_rust = Path::new("posts/long/introduction-to-rust.md");
    let about_page = Path::new("pages/about.md");

    assert!(matches(posts.as_slice(), &intro_to_rust));
    assert!(!matches(posts.as_slice(), &about_page));
  }

  #[test]
  fn match_regex() {
    let intro_to_rust = Path::new("posts/long/introduction-to-rust.md");
    let this_week_in_rust = Path::new("posts/short/this-week-in-rust.md");

    assert!(regex!(r"introduction").matches(&intro_to_rust));
    assert!(!regex!(r"introduction").matches(&this_week_in_rust));
  }

  #[test]
  fn match_conjunctions() {
    let posts = "posts/**.md";
    let intro_to_rust = Path::new("posts/long/introduction-to-rust.md");
    let this_week_in_rust = Path::new("posts/short/this-week-in-rust.md");
    let about_page = Path::new("pages/about.md");

    assert!(!and!(posts.as_slice(), not!("posts/short/this-week-in-rust.md"))
            .matches(&this_week_in_rust));
    assert!(and!(posts.as_slice(), not!("posts/short/this-week-in-rust.md"))
            .matches(&intro_to_rust));
    assert!(!and!(posts.as_slice(), not!("posts/short/this-week-in-rust.md"))
            .matches(&about_page));
  }

  #[test]
  fn match_disjunctions() {
    let intro_to_rust = Path::new("posts/long/introduction-to-rust.md");
    let about_page = Path::new("pages/about.md");

    assert!(or!("pages/about.md", "second.md").matches(&about_page));
    assert!(!or!("pages/about.md", "second.md").matches(&intro_to_rust));
  }

  #[test]
  fn not_match_negations() {
    let intro_to_rust = Path::new("posts/long/introduction-to-rust.md");
    let about_page = Path::new("pages/about.md");

    assert!(!not!("pages/about.md", "pages/lately.md").matches(&about_page));
    assert!(not!("pages/about.md", "pages/lately.md").matches(&intro_to_rust));
  }

  #[test]
  fn match_single_files() {
    let about_page = Path::new("pages/about.md");

    assert!("pages/about.md".matches(&about_page));
  }

  #[test]
  fn use_macros() {
    let posts = "posts/**.md";
    let intro_to_rust = Path::new("posts/long/introduction-to-rust.md");
    let this_week_in_rust = Path::new("posts/short/this-week-in-rust.md");
    let about_page = Path::new("pages/about.md");

    assert!(or!("pages/about.md", "pages/lately.md").matches(&about_page));
    assert!(and!(posts.as_slice(), not!("posts/short/this-week-in-rust.md"))
            .matches(&intro_to_rust));
    assert!(!and!(posts.as_slice(), not!("posts/short/this-week-in-rust.md"))
            .matches(&this_week_in_rust));

    assert!(or!("pages/about.md",
                and!("posts/**",
                     not!("posts/short/this-week-in-rust.md")))
           .matches(&intro_to_rust));

    assert!(or!("pages/about.md",
                and!("posts/**",
                     not!("posts/short/this-week-in-rust.md")))
           .matches(&about_page));

    assert!(!or!("pages/about.md",
                 and!("posts/**",
                      not!("posts/short/this-week-in-rust.md")))
           .matches(&this_week_in_rust));
  }
}
