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
#![feature(collections)]

#![plugin(regex_macros)]

extern crate glob;
extern crate anymap;
extern crate regex;
extern crate graphviz;
extern crate toml;

#[macro_use]
extern crate log;

extern crate regex_macros;

extern crate hoedown;
extern crate "rustc-serialize" as rustc_serialize;
extern crate handlebars;
extern crate docopt;

pub use diecast::Diecast;
pub use pattern::Pattern;
pub use site::{Site, Configuration, Rule};
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
pub mod diecast;
pub mod commands;

