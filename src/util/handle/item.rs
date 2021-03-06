use std::any::Any;

use typemap;

use handler::Handle;
use item::Item;
use support;

use super::Extender;

impl<T> Handle<Item> for Extender<T>
where T: typemap::Key, T::Value: Any + Sync + Send + Clone {
    fn handle(&self, item: &mut Item) -> ::Result<()> {
        item.extensions.insert::<T>(self.payload.clone());
        Ok(())
    }
}

pub fn copy(item: &mut Item) -> ::Result<()> {
    use std::fs;

    if let Some(from) = item.source() {
        if let Some(to) = item.target() {
            // TODO: once path normalization is in, make sure
            // writing to output folder

            if let Some(parent) = to.parent() {
                // TODO: this errors out if the path already exists? dumb
                support::mkdir_p(parent).unwrap();
            }

            fs::copy(from, to)?;
        }
    }

    Ok(())
}

/// Handle<Item> that reads the `Item`'s body.
pub fn read(item: &mut Item) -> ::Result<()> {
    use std::fs::File;
    use std::io::Read;

    if let Some(from) = item.source() {
        let mut buf = String::new();

        // TODO: use try!
        File::open(from)
            .unwrap()
            .read_to_string(&mut buf)
            .unwrap();

        item.body = buf;
    }

    Ok(())
}

/// Handle<Item> that writes the `Item`'s body.
pub fn write(item: &mut Item) -> ::Result<()> {
    use std::fs::File;
    use std::io::Write;

    if let Some(to) = item.target() {
        // TODO: once path normalization is in, make sure
        // writing to output folder
        if let Some(parent) = to.parent() {
            // TODO: this errors out if the path already exists? dumb
            support::mkdir_p(parent).unwrap();
        }

        // TODO: this sometimes crashes
        File::create(&to)
            .unwrap()
            .write_all(item.body.as_bytes())
            .unwrap();
    }

    Ok(())
}

