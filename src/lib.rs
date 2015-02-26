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
#![feature(fs)]
#![feature(io)]
#![feature(old_io)]
#![feature(old_path)]
#![feature(collections)]

#![plugin(regex_macros)]

extern crate glob;
extern crate anymap;
extern crate regex;
extern crate graphviz;
extern crate toml;

#[macro_use]
extern crate log;

extern crate hoedown;
extern crate "rustc-serialize" as rustc_serialize;
extern crate handlebars;
extern crate docopt;
extern crate notify;
extern crate threadpool;
// extern crate iron;
// extern crate "static" as static_file;
// extern crate mount;

pub use pattern::Pattern;
pub use site::Site;
pub use rule::Rule;
pub use configuration::Configuration;
pub use compiler::Compiler;
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
pub mod command;
pub mod configuration;
pub mod job;
pub mod rule;

