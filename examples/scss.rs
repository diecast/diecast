use std::path::PathBuf;

use diecast::{self, Handle, Bind};

pub struct Scss {
    input: PathBuf,
    output: PathBuf,
}

impl Handle<Bind> for Scss {
    fn handle(&self, bind: &mut Bind) -> diecast::Result {
        use std::process::Command;

        trace!("compiling scss");

        let source = bind.data().configuration.input.join(&self.input);
        let destination = bind.data().configuration.output.join(&self.output);

        if let Some(parent) = destination.parent() {
            diecast::mkdir_p(parent).unwrap();
        }

        let mut command = Command::new("scss");

        if let Some(load_path) = source.parent() {
            command.arg("-I").arg(load_path.to_path_buf());
        }

        command.arg(source).arg(destination);

        try!(command.status());

        Ok(())
    }
}

pub fn scss<P, Q>(input: P, output: Q) -> Scss
where P: Into<PathBuf>, Q: Into<PathBuf> {
    Scss {
        input: input.into(),
        output: output.into(),
    }
}

