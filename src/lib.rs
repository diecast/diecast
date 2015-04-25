// TODO: when ready, this prevents it from building
//       if there are missing docs or warnings
// #![warn(missing_docs)]
// #![deny(warnings)]

//! This crate facilitates the creation of static site generators.

#![feature(path_ext)]
#![feature(fs_walk)]
#![feature(path_relative_from)]
#![feature(str_char)] // for char_at
#![feature(collections)]

extern crate glob;
extern crate anymap;
extern crate regex;
extern crate toml;
extern crate threadpool;
extern crate chrono;
extern crate zmq;
extern crate websocket;
extern crate git2;

#[macro_use]
extern crate log;

#[macro_use]
extern crate hoedown;
extern crate rustc_serialize;
extern crate handlebars;
extern crate docopt;
extern crate notify;
extern crate libc;
extern crate time;
extern crate tempdir;
extern crate num_cpus;

extern crate iron;
extern crate staticfile;
extern crate mount;

pub use pattern::Pattern;
pub use site::Site;
pub use rule::Rule;
pub use configuration::Configuration;
pub use item::Item;
pub use binding::Bind;
pub use handle::{Handle, Result};

mod handle;
mod job;
mod dependency;

#[macro_use]
pub mod macros;
pub mod item;
pub mod binding;
pub mod rule;
pub mod pattern;
pub mod site;
pub mod command;
pub mod configuration;
pub mod util;
pub mod deploy;

