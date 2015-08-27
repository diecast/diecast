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

impl<P> Pattern for Box<P>
where P: Pattern {
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

// TODO
// can create an And and Or that are not type parameterized
// they would store trait objects in a vec
//
// And's implementation would be self.objs.iter().all(|p| p.matches(p))
// Or's implementation would be self.objs.iter().any(|p| p.matches(p))
//
// Would be (with appropriate macro):
//
// Or::new().add(pat1).add(pat2)
//
// Or just introduce or! etc. macro which expands to
//
// Or::from(vec![Box::new(pat1) as Box<Pattern>, Box::new(pat2) as Box<Pattern>])

/// This conjunction of two patterns.
pub struct And<A, B>
where A: Pattern, B: Pattern {
    left: A,
    right: B
}

impl<A, B> Pattern for And<A, B>
where A: Pattern, B: Pattern {
    fn matches(&self, p: &Path) -> bool {
        self.left.matches(p) && self.right.matches(p)
    }
}

/// The disjunction of two patterns.
pub struct Or<A, B>
where A: Pattern, B: Pattern {
    left: A,
    right: B
}

impl<A, B> Pattern for Or<A, B>
where A: Pattern, B: Pattern {
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

/// Pattern that matches nothing.
#[derive(Copy, Clone)]
pub struct Nothing;

impl Pattern for Nothing {
    fn matches(&self, _: &Path) -> bool {
        false
    }
}

/// Allow regular expression patterns.
impl Pattern for Regex {
    fn matches(&self, p: &Path) -> bool {
        p.to_str()
            .map_or(false, |s| self.is_match(s))
    }
}

/// Treat string slices as literal patterns.
impl Pattern for str {
    fn matches(&self, p: &Path) -> bool {
        p.to_str().map_or(false, |s| self == s)
    }
}

impl Pattern for Path {
    fn matches(&self, p: &Path) -> bool {
        self == p
    }
}

impl Pattern for HashSet<PathBuf> {
    fn matches(&self, p: &Path) -> bool {
        // FIXME rust 1.0
        // https://github.com/rust-lang/rust/pull/25060
        self.contains(&p.to_path_buf())
    }
}

impl Pattern for glob::Pattern {
    fn matches(&self, p: &Path) -> bool {
        self.matches_path(p)
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
    pub fn and<A, B>(a: A, b: B) -> And<A, B>
    where A: Pattern, B: Pattern {
        And {
            left: a,
            right: b
        }
    }

    /// Constructs the disjunction of two patterns.
    pub fn or<A, B>(a: A, b: B) -> Or<A, B>
    where A: Pattern, B: Pattern {
        Or {
            left: a,
            right: b
        }
    }
}

#[cfg(test)]
mod test {
    use regex::Regex;

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

        assert!(Pattern::matches("pages/about.md", &about_page));
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
