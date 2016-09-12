#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

//! This crate facilitates the creation of static site generators.

// TODO: when ready, this prevents it from building
//       if there are missing docs or warnings
// #![warn(missing_docs)]
// #![deny(warnings)]

extern crate glob;
extern crate regex;
extern crate toml;
extern crate typemap;
extern crate walkdir;
extern crate time;

#[macro_use]
extern crate log;

extern crate rustc_serialize;
extern crate docopt;
extern crate num_cpus;
extern crate ansi_term;

extern crate futures;
extern crate futures_cpupool;

pub use pattern::Pattern;
pub use site::Site;
pub use rule::Rule;
pub use configuration::Configuration;
pub use item::Item;
pub use bind::Bind;
pub use handler::Handle;
// TODO command hooks
pub use command::Command;

mod handler;
mod job;
mod dependency;

#[macro_use]
pub mod macros;
pub mod item;
pub mod bind;
pub mod rule;
pub mod pattern;
pub mod site;
pub mod command;
pub mod configuration;
pub mod util;
pub mod support;

pub type Error = Box<::std::error::Error + Sync + Send>;
pub type Result<T> = ::std::result::Result<T, Error>;
