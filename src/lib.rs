// TODO: when ready, this prevents it from building
//       if there are missing docs or warnings
// #![deny(missing_doc)]
// #![deny(warnings)]

//! This crate facilitates the creation of static site generators.

#![feature(plugin)]
#![feature(core)]
#![feature(rustc_private)]
#![feature(std_misc)]
#![feature(unsafe_destructor)]
#![feature(path_ext)]
#![feature(convert)]
#![feature(fs_walk)]
#![feature(path_relative_from)]
#![feature(str_char)]
#![feature(into_cow)]
#![feature(collections)]

#![plugin(regex_macros)]

extern crate glob;
extern crate anymap;
extern crate regex;
extern crate graphviz;
extern crate toml;
extern crate threadpool;

#[macro_use]
extern crate log;

extern crate hoedown;
extern crate rustc_serialize;
extern crate handlebars;
extern crate docopt;
extern crate notify;
extern crate libc;
extern crate time;
extern crate tempdir;
extern crate num_cpus;
// extern crate iron;
// extern crate "static" as static_file;
// extern crate mount;

pub use pattern::Pattern;
pub use site::Site;
pub use rule::Rule;
pub use configuration::Configuration;
pub use item::{Item, Dependencies};
pub use binding::Bind;

#[macro_use]
pub mod macros;
pub mod deploy;
pub mod pattern;
pub mod item;
pub mod binding;
pub mod router;
pub mod compiler;
pub mod site;
pub mod dependency;
pub mod command;
pub mod configuration;
pub mod job;
pub mod rule;

fn exit(code: usize) -> ! {
    unsafe { libc::exit(code as libc::c_int) }
}
