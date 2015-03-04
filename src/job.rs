use std::fmt;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Sender, SendError, Receiver, RecvError};
use std::thread;

use compiler::{Compile, is_paused};
use item::Item;

pub struct Job {
    pub id: usize,
    pub binding: &'static str,

    pub item: Item,
    pub compiler: Arc<Box<Compile>>,
    pub dependency_count: usize,

    pub is_paused: bool,
}

impl fmt::Debug for Job {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}. [{}]: {:?}",
               self.id,
               self.binding,
               self.item)
    }
}

impl Job {
    pub fn new(
        binding: &'static str,
        item: Item,
        compiler: Arc<Box<Compile>>,
        id: usize)
    -> Job {
        Job {
            id: id,
            binding: binding,
            item: item,
            compiler: compiler,
            dependency_count: 0,
            is_paused: false,
        }
    }

    pub fn process(mut self, tx: Sender<Result<Job, Error>>) {
        match self.compiler.compile(&mut self.item) {
            Ok(()) => {
                // TODO: we're still special-casing Chain here, doesn't matter?
                self.is_paused = is_paused(&self.item);

                tx.send(Ok(self)).unwrap()
            },
            Err(e) => {
                println!("\nthe following job encountered an error:\n  {:?}\n\n{}\n", self, e);
                tx.send(Err(Error::Err)).unwrap();
            }
        }
    }
}

pub enum Error {
    Err,
    Panic,
}

struct Sentinel {
    tx: Sender<Result<Job, Error>>,
    active: bool
}

impl Sentinel {
    fn new(tx: Sender<Result<Job, Error>>) -> Sentinel {
        Sentinel {
            tx: tx,
            active: true
        }
    }

    // Cancel and destroy this sentinel.
    fn cancel(mut self) {
        self.active = false;
    }
}

#[unsafe_destructor]
impl Drop for Sentinel {
    fn drop(&mut self) {
        if self.active {
            match self.tx.send(Err(Error::Panic)) {
                Ok(_) => (), // will close down everything
                Err(_) => (), // already pannicked once
            }
        }
    }
}

pub struct Pool {
    // How the threadpool communicates with subthreads.
    //
    // This is the only such Sender, so when it is dropped all subthreads will
    // quit.
    enqueue: Sender<Job>,
    dequeue: Receiver<Result<Job, Error>>,
}

impl Pool {
    /// Spawns a new thread pool with `threads` threads.
    ///
    /// # Panics
    ///
    /// This function will panic if `threads` is 0.
    pub fn new(threads: usize) -> Pool {
        assert!(threads >= 1);

        let (enqueue, rx) = channel::<Job>();
        let rx = Arc::new(Mutex::new(rx));
        let (tx, dequeue) = channel::<Result<Job, Error>>();

        // Threadpool threads
        for _ in 0..threads {
            let rx = rx.clone();
            let tx = tx.clone();

            thread::spawn(move || {
                // Will spawn a new thread on panic unless it is cancelled.
                let sentinel = Sentinel::new(tx.clone());

                loop {
                    let message = {
                        // Only lock jobs for the time it takes
                        // to get a job, not run it.
                        let lock = rx.lock().unwrap();
                        lock.recv()
                    };

                    match message {
                        Ok(job) => {
                            job.process(tx.clone());
                        },

                        // The Taskpool was dropped.
                        Err(..) => break
                    }
                }

                sentinel.cancel();
            });
        }

        Pool {
            enqueue: enqueue,
            dequeue: dequeue,
        }
    }

    pub fn enqueue(&self, job: Job) -> Result<(), SendError<Job>> {
        self.enqueue.send(job)
    }

    pub fn dequeue(&self) -> Result<Result<Job, Error>, RecvError> {
        self.dequeue.recv()
    }
}
