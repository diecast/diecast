use std::fs;
use std::path::Path;
use std::io;

// TODO
// remove this and use create_dir_all?
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
