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
//!```ignore
//!let pat =
//!    or!(
//!        "posts/**",
//!        and!(
//!            "pages/**",
//!            not!("pages/secret-work.md")))
//!);
//!```

use glob;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::collections::HashSet;

/// A kind of pattern that can be used for
/// filtering the files in the input directory.
pub trait Pattern {
    fn matches(&self, &Path) -> bool;
}

impl<P> Pattern for Box<P> where P: Pattern {
    fn matches(&self, path: &Path) -> bool {
        (**self).matches(path)
    }
}

impl<'a, P: ?Sized> Pattern for &'a P where P: Pattern {
    fn matches(&self, path: &Path) -> bool {
        (**self).matches(path)
    }
}

impl<'a, P: ?Sized> Pattern for &'a mut P where P: Pattern {
    fn matches(&self, path: &Path) -> bool {
        (**self).matches(path)
    }
}

/// The negation of a pattern.
pub struct Not<P>
where P: Pattern {
    pattern: P
}

impl<P> Pattern for Not<P>
where P: Pattern {
    fn matches(&self, p: &Path) -> bool {
        !self.pattern.matches(p)
    }
}

/// This conjunction of two patterns.
pub struct And<P1, P2>
where P1: Pattern, P2: Pattern {
    left: P1,
    right: P2
}

impl<P1, P2> Pattern for And<P1, P2>
where P1: Pattern, P2: Pattern {
    fn matches(&self, p: &Path) -> bool {
        self.left.matches(p) && self.right.matches(p)
    }
}

/// The disjunction of two patterns.
pub struct Or<P1, P2>
where P1: Pattern, P2: Pattern {
    left: P1,
    right: P2
}

impl<P1, P2> Pattern for Or<P1, P2>
where P1: Pattern, P2: Pattern {
    fn matches(&self, p: &Path) -> bool {
        self.left.matches(p) || self.right.matches(p)
    }
}

/// Pattern that matches everything.
#[derive(Copy, Clone)]
pub struct Everything;

impl Pattern for Everything {
    fn matches(&self, _: &Path) -> bool {
        true
    }
}

/// Allow regular expression patterns.
impl Pattern for Regex {
    fn matches(&self, p: &Path) -> bool {
        self.is_match(p.to_str().unwrap())
    }
}

// TODO: consider making &str impl be exact match?
/// Treat string slices as globs.
///
/// This simply converts the string to glob::Pattern.
/// It's much more efficient to just use the glob::Pattern
/// to begin with.
impl Pattern for str {
    fn matches(&self, p: &Path) -> bool {
        self == p.to_str().unwrap()
    }
}

impl Pattern for Path {
    fn matches(&self, p: &Path) -> bool {
        self == p
    }
}

impl Pattern for HashSet<PathBuf> {
    fn matches(&self, p: &Path) -> bool {
        // FIXME: upon https://github.com/rust-lang/rust/issues/23261
        self.contains(&p.to_path_buf())
    }
}

impl Pattern for glob::Pattern {
    // TODO: glob should be updated to work on Path
    fn matches(&self, p: &Path) -> bool {
        self.matches(p.to_str().unwrap())
    }
}

/// Contains the DSL items for easily constructing complex patterns.
pub mod dsl {
    use super::{Pattern, Not, And, Or};

    /// Constructs the negation of a pattern.
    pub fn not<P>(p: P) -> Not<P>
    where P: Pattern {
        Not {
            pattern: p
        }
    }

    /// Constructs the conjunction of two patterns.
    pub fn and<P1, P2>(p1: P1, p2: P2) -> And<P1, P2>
    where P1: Pattern, P2: Pattern {
        And {
            left: p1,
            right: p2
        }
    }

    /// Constructs the disjunction of two patterns.
    pub fn or<P1, P2>(p1: P1, p2: P2) -> Or<P1, P2>
    where P1: Pattern, P2: Pattern {
        Or {
            left: p1,
            right: p2
        }
    }
}

#[cfg(test)]
mod test {
    use super::{Pattern, Everything};
    use std::path::Path;

    #[test]
    fn match_everything() {
        let intro_to_rust = Path::new("posts/long/introduction-to-rust.md");

        assert!(pattern!(Everything).matches(&intro_to_rust));
    }

    #[test]
    fn match_globs() {
        use glob;

        let pattern = glob::Pattern::new("posts/**/*.md").unwrap();
        let intro_to_rust = Path::new("posts/long/introduction-to-rust.md");
        let about_page = Path::new("pages/about.md");

        assert!(Pattern::matches(&pattern, &intro_to_rust));
        assert!(!Pattern::matches(&pattern, &about_page));
    }

    #[test]
    fn match_regex() {
        let intro_to_rust = Path::new("posts/long/introduction-to-rust.md");
        let this_week_in_rust = Path::new("posts/short/this-week-in-rust.md");

        assert!(Regex::new(r"introduction").unwrap().matches(&intro_to_rust));
        assert!(!Regex::new(r"introduction").unwrap().matches(&this_week_in_rust));
    }

    #[test]
    fn match_conjunctions() {
        use glob;

        let posts = glob::Pattern::new("posts/**/*.md").unwrap();
        let intro_to_rust = Path::new("posts/long/introduction-to-rust.md");
        let this_week_in_rust = Path::new("posts/short/this-week-in-rust.md");
        let about_page = Path::new("pages/about.md");

        assert!(!and!(&posts, not!("posts/short/this-week-in-rust.md"))
                .matches(&this_week_in_rust));
        assert!(and!(&posts, not!("posts/short/this-week-in-rust.md"))
                .matches(&intro_to_rust));
        assert!(!and!(&posts, not!("posts/short/this-week-in-rust.md"))
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
        use glob;

        let posts = glob::Pattern::new("posts/**/*.md").unwrap();
        let posts_level = glob::Pattern::new("posts/**").unwrap();
        let intro_to_rust = Path::new("posts/long/introduction-to-rust.md");
        let this_week_in_rust = Path::new("posts/short/this-week-in-rust.md");
        let about_page = Path::new("pages/about.md");

        assert!(or!("pages/about.md", "pages/lately.md").matches(&about_page));
        assert!(and!(&posts, not!("posts/short/this-week-in-rust.md"))
                .matches(&intro_to_rust));
        assert!(!and!(&posts, not!("posts/short/this-week-in-rust.md"))
                .matches(&this_week_in_rust));

        assert!(or!("pages/about.md",
                    and!(&posts_level,
                         not!("posts/short/this-week-in-rust.md")))
                .matches(&intro_to_rust));

        assert!(or!("pages/about.md",
                    and!(&posts_level,
                         not!("posts/short/this-week-in-rust.md")))
                .matches(&about_page));

        assert!(!or!("pages/about.md",
                     and!(&posts_level,
                          not!("posts/short/this-week-in-rust.md")))
                .matches(&this_week_in_rust));
    }
}
