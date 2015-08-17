use std::sync::Arc;
use std::fmt;

use time::PreciseTime;

use bind::{self, Bind};
use handler::Handle;

pub mod evaluator;
mod manager;

pub use self::evaluator::Evaluator;
pub use self::manager::Manager;

pub static STARTING: &'static str = "  Starting";
pub static FINISHED: &'static str = "  Finished";

pub struct Job {
    pub handler: Arc<Box<Handle<Bind> + Sync + Send>>,
    pub bind: bind::Data,
}

impl fmt::Debug for Job {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{}]", self.bind.name)
    }
}

impl Job {
    pub fn new(
        bind: bind::Data,
        handler: Arc<Box<Handle<Bind> + Sync + Send>>)
    -> Job {
        Job {
            handler: handler,
            bind: bind,
        }
    }

    pub fn process(self) -> ::Result<Bind> {
        use ansi_term::Colour::Green;
        use ansi_term::Style;

        let mut bind = Bind::new(self.bind);

        println!("{} {}",
            Green.bold().paint(STARTING),
            bind);

        let start = PreciseTime::now();
        let res = self.handler.handle(&mut bind);
        let end = PreciseTime::now();

        let duration = start.to(end);

        println!("{} {} [{}] {}",
            Style::default().bold().paint(FINISHED),
            bind,
            bind.items().len(),
            duration);

        match res {
            Ok(_) => Ok(bind),
            Err(e) =>
                Err(From::from(
                    format!("\nthe following job encountered an error:\n  {:?}\n\n{}\n",
                            bind.name,
                            e))),
        }
    }
}
