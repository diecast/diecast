#![crate_name = "diecast"]

// TODO: when ready, this prevents it from building
//       if there are missing docs or warnings
// #![deny(missing_doc)]
// #![deny(warnings)]

//! This crate facilitates the creation of static site generators.

#![feature(macro_rules)]
#![feature(phase)]
#![feature(default_type_params)]
#![feature(globs)]
#![feature(unboxed_closures)]
#![feature(slicing_syntax)]

extern crate glob;
extern crate anymap;
extern crate regex;
extern crate graphviz;

#[phase(plugin)]
extern crate regex_macros;

pub use pattern::Pattern;
pub use generator::{Generator, Processor};
pub use compiler::{Compiler, Chain};
pub use item::{Item, Dependencies};

pub mod macros;
pub mod deploy;
pub mod pattern;
pub mod item;
pub mod router;
pub mod compiler;
pub mod generator;
pub mod dependency;

// for macros
mod diecast {
  pub use pattern;
}

