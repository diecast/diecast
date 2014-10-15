#![macro_escape]

/*
 * TODO: when this is made a library,
 * use absolute path in use statement
 * use ::mycratename::pattern::dsl::*;
 * and in the crate root (lib.rs?)
 * pub use pattern;
 */

/// This macro simply brings the dsl module's contents
/// within the scope of the expression passed to it.
///
/// That is, you can do this:
///
/// ```ignore
/// pattern!(and("posts/**", not("posts/badfile.txt")))
/// ```
///
/// Instead of:
///
/// ```ignore
/// let pattern = {
///   use diecast::pattern::dsl::*;
///
///   and("posts/**", not("posts/badfile.txt"))
/// };
/// ```
#[macro_export]
macro_rules! pattern {
  ($pat:expr) => {
    {
      use diecast::pattern::dsl::*;
      $pat
    }
  };
}

/// Helper macro for constructing variadic macros.
#[macro_export]
macro_rules! variadic {
  ($op:path, $e:expr) => {$e};

  ($op:path, $head:expr, $($tail:expr),+) => {
    $op($head, variadic!($op, $($tail),+))
  };
}

/// Constructs an `OrPattern` out of variable arguments.
#[macro_export]
macro_rules! or {
  ($($e:expr),+) => {variadic!(::diecast::pattern::dsl::or, $($e),+)};
}

/// Constructs an `AndPattern` out of variable arguments.
#[macro_export]
macro_rules! and {
  ($($e:expr),+) => {variadic!(::diecast::pattern::dsl::and, $($e),+)};
}

/// Constructs a `NotPattern` out of variable arguments.
#[macro_export]
macro_rules! not {
  ($e:expr) => {
    ::diecast::pattern::dsl::not($e)
  };

  ($head:expr, $($tail:expr),+) => {
    ::diecast::pattern::dsl::and(::diecast::pattern::dsl::not($head), not!($($tail),+))
  };
}
