//! Deployment behavior.

/// This can be implemented and used to handle
/// the deployment of the generated site.
pub trait Deploy {
  fn run(&self);
}

#[deriving(Copy)]
pub struct DoNothing;

impl Deploy for DoNothing {
  fn run(&self) {
    println!("no deploy command is registered");
  }
}

