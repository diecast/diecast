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
use regex;

/// A kind of pattern that can be used for
/// filtering the files in the input directory.
pub trait Pattern {
  fn matches(&self, &Path) -> bool;
}

impl Pattern for Box<Pattern + Send + Sync> {
  fn matches(&self, p: &Path) -> bool {
    (**self).matches(p)
  }
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
pub struct Everything;

impl Pattern for Everything {
  fn matches(&self, _: &Path) -> bool {
    true
  }
}

/// Allow regular expression patterns.
impl Pattern for regex::Regex {
  fn matches(&self, p: &Path) -> bool {
    self.is_match(p.as_str().unwrap())
  }
}

/// Treat string slices as globs.
impl<'a> Pattern for &'a str {
  fn matches(&self, p: &Path) -> bool {
    glob::Pattern::new(*self).matches_path(p)
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
  pub use super::{Pattern, Everything};

  pub fn matches<T>(pattern: T, p: &Path) -> bool
    where T: Pattern {
    pattern.matches(p)
  }

  describe! dsl {
    before_each {
      let posts = "posts/**.md";

      let intro_to_rust = Path::new("posts/long/introduction-to-rust.md");
      let this_week_in_rust = Path::new("posts/short/this-week-in-rust.md");
      let about_page = Path::new("pages/about.md");
    }

    it "should match everything" {
      assert!(pattern!(Everything).matches(&intro_to_rust));
    }

    it "should match globs" {
      assert!(matches(posts.as_slice(), &intro_to_rust));
      assert!(!matches(posts.as_slice(), &about_page));
    }

    it "should match regular expressions" {
      assert!(regex!(r"introduction").matches(&intro_to_rust));
      assert!(!regex!(r"introduction").matches(&this_week_in_rust));
    }

    it "should match conjunctions" {
      assert!(!and!(posts.as_slice(), not!("posts/short/this-week-in-rust.md"))
              .matches(&this_week_in_rust));
      assert!(and!(posts.as_slice(), not!("posts/short/this-week-in-rust.md"))
              .matches(&intro_to_rust));
      assert!(!and!(posts.as_slice(), not!("posts/short/this-week-in-rust.md"))
              .matches(&about_page));
    }

    it "should match disjunctions" {
      assert!(or!("pages/about.md", "second.md").matches(&about_page));
      assert!(!or!("pages/about.md", "second.md").matches(&intro_to_rust));
    }

    it "should not match negations" {
      assert!(!not!("pages/about.md", "pages/lately.md").matches(&about_page));
      assert!(not!("pages/about.md", "pages/lately.md").matches(&intro_to_rust));
    }

    it "should match single files" {
      assert!("pages/about.md".matches(&about_page));
    }

    it "should work with the macros" {
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
}
