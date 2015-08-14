use std::fs::{self, PathExt};
use std::path::Path;
use std::io;

pub fn mkdir_p<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = path.as_ref();
    if path == Path::new("") || ::std::fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false) { return Ok(()) }
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

pub fn file_exists<P: AsRef<Path>>(path: P) -> bool {
    ::std::fs::metadata(path).is_ok()
}

pub fn path_relative_from<'a, P: ?Sized + AsRef<Path>, R: ?Sized + AsRef<Path>>(target: &'a R, base: &'a P) -> Option<&'a Path> {
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

pub fn slugify(s: &str) -> String {
    s.chars()
    .filter_map(|c| {
        let is_ws = c.is_whitespace();
        if c.is_alphanumeric() || is_ws {
            let c = c.to_lowercase().next().unwrap();
            if is_ws { Some('-') }
            else { Some(c) }
        } else {
            None
        }
    })
    .collect()
}
