use std::fmt;
use std::mem;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread::{self, JoinGuard};

use compiler::{self, Compile, is_paused};
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
        // FIXME: this should actually be returned
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

trait FnBox {
    fn call_box(self: Box<Self>);
}

impl<F: FnOnce()> FnBox for F {
    fn call_box(self: Box<F>) {
        (*self)()
    }
}

type Thunk<'a> = Box<FnBox + Send + 'a>;

struct Sentinel<'a> {
    jobs: &'a Arc<Mutex<Receiver<Thunk<'static>>>>,
    tx: Sender<Result<Job, Error>>,
    active: bool
}

impl<'a> Sentinel<'a> {
    fn new(jobs: &'a Arc<Mutex<Receiver<Thunk<'static>>>>, tx: Sender<Result<Job, Error>>) -> Sentinel<'a> {
        Sentinel {
            jobs: jobs,
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
impl<'a> Drop for Sentinel<'a> {
    fn drop(&mut self) {
        if self.active {
            match self.tx.send(Err(Error::Panic)) {
                Ok(_) => (), // will close down everything
                Err(_) => (), // already pannicked once
            }
            // spawn_in_pool(self.jobs.clone(), self.tx.clone())
        }
    }
}

/// A thread pool used to execute functions in parallel.
///
/// Spawns `n` worker threads and replenishes the pool if any worker threads
/// panic.
///
/// # Example
///
/// ```rust
/// use threadpool::ThreadPool;
/// use std::sync::mpsc::channel;
///
/// let pool = ThreadPool::new(4);
///
/// let (tx, rx) = channel();
/// for i in 0..8 {
///     let tx = tx.clone();
///     pool.execute(move|| {
///         tx.send(i).unwrap();
///     });
/// }
///
/// assert_eq!(rx.iter().take(8).fold(0, |a, b| a + b), 28);
/// ```
pub struct ThreadPool {
    // How the threadpool communicates with subthreads.
    //
    // This is the only such Sender, so when it is dropped all subthreads will
    // quit.
    jobs: Sender<Thunk<'static>>,
    tx: Sender<Result<Job, Error>>,
}

impl ThreadPool {
    /// Spawns a new thread pool with `threads` threads.
    ///
    /// # Panics
    ///
    /// This function will panic if `threads` is 0.
    pub fn new(threads: usize, tx_: Sender<Result<Job, Error>>) -> ThreadPool {
        assert!(threads >= 1);

        println!("threadpool with {} threads", threads);

        let (tx, rx) = channel::<Thunk<'static>>();
        let rx = Arc::new(Mutex::new(rx));

        // Threadpool threads
        for _ in 0..threads {
            spawn_in_pool(rx.clone(), tx_.clone());
        }

        ThreadPool { jobs: tx, tx: tx_ }
    }

    /// Executes the function `job` on a thread in the pool.
    pub fn execute<F>(&self, job: F)
        where F : FnOnce() + Send + 'static
    {
        self.jobs.send(Box::new(move || job())).unwrap();
    }
}

fn spawn_in_pool(jobs: Arc<Mutex<Receiver<Thunk<'static>>>>, tx: Sender<Result<Job, Error>>) {
    thread::spawn(move || {
        // Will spawn a new thread on panic unless it is cancelled.
        let sentinel = Sentinel::new(&jobs, tx);

        loop {
            let message = {
                // Only lock jobs for the time it takes
                // to get a job, not run it.
                let lock = jobs.lock().unwrap();
                lock.recv()
            };

            match message {
                Ok(job) => {
                    job.call_box();
                },

                // The Taskpool was dropped.
                Err(..) => break
            }
        }

        sentinel.cancel();
    });
}
