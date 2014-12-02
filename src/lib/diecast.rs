#![crate_name = "diecast"]
#![comment = "Language-Agnostic Static Site Generator in Rust"]
#![license = "BSD"]

// TODO: when ready, this prevents it from building
//       if there are missing docs or warnings
// #![deny(missing_doc)]
// #![deny(warnings)]

//! This crate facilitates the creation of static site generators.

#![feature(tuple_indexing)]
#![feature(macro_rules)]
#![feature(phase)]
#![feature(default_type_params)]
#![feature(if_let)]
#![feature(while_let)]
#![feature(globs)]
#![feature(unboxed_closures)]
#![feature(slicing_syntax)]

extern crate glob;
extern crate anymap;
extern crate regex;
extern crate graphviz;

#[phase(plugin)]
extern crate regex_macros;

// #[phase(plugin, link)]
// extern crate stainless;

pub use pattern::Pattern;
pub use generator::Generator;
pub use compile::Compile;
pub use item::Item;

pub mod macros;
pub mod deploy;
pub mod pattern;
pub mod item;
pub mod route;
pub mod compile;
pub mod generator;
pub mod dependency;

// for macros
mod diecast {
  pub use pattern;
}

