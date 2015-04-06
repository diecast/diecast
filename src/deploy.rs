//! Deployment behavior.

/// This can be implemented and used to handle
/// the deployment of the generated site.
pub trait Deploy {
    fn run(&self);
}

// TODO: need impls for box, ref, and ref mut as with Compile & Deploy

#[derive(Copy, Clone)]
pub struct DoNothing;

impl Deploy for DoNothing {
    fn run(&self) {
        println!("no deploy command is registered");
    }
}

