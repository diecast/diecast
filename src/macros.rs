#[macro_export]
macro_rules! rule {
    (name: $name:expr,
     pattern: $pattern:expr,
     handler: $handler:expr) => {
        $crate::rule::Rule::named($name)
            .matching($pattern)
            .handler($handler)
            .build()
    };

    (name: $name:expr,
     dependencies: [$($dependency:expr),+],
     handler: $handler:expr) => {
        $crate::rule::Rule::named($name)
            $(.depends_on($dependency))+
            .handler($handler)
            .build()
    };

    (name: $name:expr,
     handler: $handler:expr) => {
        $crate::rule::Rule::named($name)
            .handler($handler)
            .build()
    };

    (name: $name:expr,
     dependencies: [$($dependency:expr),+],
     pattern: $pattern:expr,
     handler: $handler:expr) => {
        $crate::rule::Rule::named($name)
            $(.depends_on($dependency))+
            .matching($pattern)
            .handler($handler)
            .build()
    }
}

#[macro_export]
macro_rules! chain {
    ($($handler:expr),+) => {
        $crate::util::handle::Chain::new()$(.link($handler))+
    };
}

#[macro_export]
macro_rules! glob {
    ($string:expr) => {
        // TODO how to reference glob?
        ::glob::Pattern::new($string).unwrap()
    }
}

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
///```ignore
///let pattern = {
///    use diecast::pattern::dsl::*;
///
///    and("posts/**", not("posts/badfile.txt"))
///};
///```
#[macro_export]
macro_rules! pattern {
    ($pat:expr) => {
        {
            #[allow(unused_imports)]
            use $crate::pattern::dsl::*;
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
    ($($e:expr),+) => {variadic!($crate::pattern::dsl::or, $($e),+)};
}

/// Constructs an `AndPattern` out of variable arguments.
#[macro_export]
macro_rules! and {
    ($($e:expr),+) => {variadic!($crate::pattern::dsl::and, $($e),+)};
}

/// Constructs a `NotPattern` out of variable arguments.
#[macro_export]
macro_rules! not {
    ($e:expr) => {
        $crate::pattern::dsl::not($e)
    };

    ($head:expr, $($tail:expr),+) => {
        $crate::pattern::dsl::and($crate::pattern::dsl::not($head), not!($($tail),+))
    };
}
