// TODO: when ready, this prevents it from building
//       if there are missing docs or warnings
// #![warn(missing_docs)]
#![deny(warnings)]

//! This crate facilitates the creation of static site generators.

extern crate glob;
extern crate regex;
extern crate toml;
extern crate threadpool;
extern crate chrono;
extern crate typemap;
extern crate walker;

#[macro_use]
extern crate log;

extern crate rustc_serialize;
extern crate docopt;
extern crate notify;
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

fn mkdir_p<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = path.as_ref();
    if path == Path::new("") || ::std::fs::metadata(path).unwrap().is_dir() { return Ok(()) }
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

fn file_exists<P: AsRef<Path>>(path: P) -> bool {
    ::std::fs::metadata(path).is_ok()
}

fn path_relative_from<'a, P: ?Sized + AsRef<Path>, R: ?Sized + AsRef<Path>>(target: &'a R, base: &'a P) -> Option<&'a Path> {
    iter_after(target.as_ref().components(), base.as_ref().components()).map(|c| c.as_path())
}

fn iter_after<A, I, J>(mut iter: I, mut prefix: J) -> Option<I> where
    I: Iterator<Item=A> + Clone, J: Iterator<Item=A>, A: PartialEq
{
    loop {
        let mut iter_next = iter.clone();
        match (iter_next.next(), prefix.next()) {
            (Some(x), Some(y)) => {
                if x != y { return None }
            }
            (Some(_), None) => return Some(iter),
            (None, None) => return Some(iter),
            (None, Some(_)) => return None,
        }
        iter = iter_next;
    }
}
