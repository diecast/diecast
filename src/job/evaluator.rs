use super::Job;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::collections::VecDeque;

use threadpool::ThreadPool;

pub trait Evaluator {
    fn enqueue(&mut self, job: Job);
    fn dequeue(&mut self) -> Option<Job>;
}

struct Canary<T> where T: Send {
    tx: Sender<Option<T>>,
    active: bool
}

impl<T> Canary<T> where T: Send {
    fn new(tx: Sender<Option<T>>) -> Canary<T> {
        Canary {
            tx: tx,
            active: true,
        }
    }

    fn cancel(mut self) {
        self.active = false;
    }
}

#[unsafe_destructor]
impl<T> Drop for Canary<T> where T: Send {
    fn drop(&mut self) {
        if self.active {
            self.tx.send(None).unwrap();
        }
    }
}

pub struct Pool<T> where T: Send {
    result_tx: Sender<Option<T>>,
    result_rx: Receiver<Option<T>>,

    pool: ThreadPool,
}

impl<T> Pool<T> where T: Send {
    pub fn new(threads: usize) -> Pool<T> {
        assert!(threads >= 1);
        trace!("using {} threads", threads);

        let (result_tx, result_rx) = channel::<Option<T>>();

        let pool = ThreadPool::new(threads);

        Pool {
            result_tx: result_tx,
            result_rx: result_rx,

            pool: pool,
        }
    }

    pub fn enqueue<F>(&self, work: F)
    where T: 'static,
          F: FnOnce() -> Option<T>, F: Send + 'static {
        let tx = self.result_tx.clone();

        self.pool.execute(move || {
            let canary = Canary::new(tx.clone());

            tx.send(work()).unwrap();

            canary.cancel();
        });
    }

    pub fn dequeue(&self) -> Option<T> {
        self.result_rx.recv().unwrap()
    }
}

impl Evaluator for Pool<Job> {
    fn enqueue(&mut self, job: Job) {
        <Pool<Job>>::enqueue(self, move || {
            let mut job = job;

            match job.process() {
                Ok(()) => Some(job),
                Err(e) => {
                    println!("\nthe following job encountered an error:\n  {:?}\n\n{}\n", job, e);
                    None
                },
            }
        });
    }

    fn dequeue(&mut self) -> Option<Job> {
        <Pool<Job>>::dequeue(self)
    }
}

impl Evaluator for VecDeque<Job> {
    fn enqueue(&mut self, job: Job) {
        self.push_back(job);
    }
    fn dequeue(&mut self) -> Option<Job> {
        self.pop_front().and_then(|mut job| {
            match job.process() {
                Ok(()) => Some(job),
                Err(e) => {
                    println!("\nthe following job encountered an error:\n  {:?}\n\n{}\n", job, e);
                    None
                },
            }
        })
    }
}

