// TODO: when ready, this prevents it from building
//       if there are missing docs or warnings
// #![warn(missing_docs)]
// #![deny(warnings)]

//! This crate facilitates the creation of static site generators.

#![feature(path_ext)]
#![feature(fs_walk)]
#![feature(path_relative_from)]
#![feature(collections)]

extern crate glob;
extern crate regex;
extern crate toml;
extern crate threadpool;
extern crate chrono;
extern crate zmq;
extern crate websocket;
extern crate git2;
extern crate typemap;

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
pub use source::Source;
// TODO command hooks
pub use command::Command;
pub use deploy::Deploy;

mod handle;
mod job;
mod dependency;
mod source;

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

use std::fs::{self, PathExt};
use std::path::Path;
use std::io;

pub fn mkdir_p<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = path.as_ref();
    if path == Path::new("") || path.is_dir() { return Ok(()) }
    if let Some(p) = path.parent() { try!(mkdir_p(p)) }
    match fs::create_dir(path) {
        Ok(()) => {
            Ok(())
        },
        Err(e) => {
            if let ::std::io::ErrorKind::AlreadyExists = e.kind() {
                Ok(())
            } else {
                return Err(e)
            }
        },
    }
}

