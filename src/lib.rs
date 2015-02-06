// TODO: when ready, this prevents it from building
//       if there are missing docs or warnings
// #![deny(missing_doc)]
// #![deny(warnings)]

//! This crate facilitates the creation of static site generators.

#![feature(plugin)]
#![feature(core)]
#![feature(rustc_private)]
#![feature(std_misc)]
#![feature(path)]
#![feature(os)]
#![feature(io)]

extern crate glob;
extern crate anymap;
extern crate regex;
extern crate graphviz;
extern crate toml;

#[plugin]
extern crate regex_macros;

pub use pattern::Pattern;
pub use site::{Site, Rule};
pub use compiler::{Compiler, Chain};
pub use item::{Item, Dependencies};

#[macro_use]
pub mod macros;
pub mod deploy;
pub mod pattern;
pub mod item;
pub mod router;
pub mod compiler;
pub mod site;
pub mod dependency;

