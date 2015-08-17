use super::Job;
use bind::Bind;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::collections::VecDeque;

use threadpool::ThreadPool;

pub trait Evaluator {
    fn enqueue(&mut self, job: Job);
    fn dequeue(&mut self) -> Option<Bind>;
}

struct Canary<T>
where T: Send {
    tx: Sender<Option<T>>,
    active: bool
}

impl<T> Canary<T>
where T: Send {
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

impl<T> Drop for Canary<T>
where T: Send {
    fn drop(&mut self) {
        if self.active {
            self.tx.send(None).unwrap();
        }
    }
}

pub struct Pool {
    result_tx: Sender<Option<Bind>>,
    result_rx: Receiver<Option<Bind>>,

    pool: ThreadPool,
}

impl Pool {
    pub fn new(threads: usize) -> Pool {
        assert!(threads >= 1);

        let (result_tx, result_rx) = channel::<Option<Bind>>();

        let pool = ThreadPool::new(threads);

        Pool {
            result_tx: result_tx,
            result_rx: result_rx,

            pool: pool,
        }
    }

    // TODO
    // Option<Bind> retval is a hack
    pub fn enqueue<F>(&self, work: F)
    where F: FnOnce() -> Option<Bind>, F: Send + 'static {
        let tx = self.result_tx.clone();

        self.pool.execute(move || {
            let canary = Canary::new(tx.clone());

            tx.send(work()).unwrap();

            canary.cancel();
        });
    }

    pub fn dequeue(&self) -> Option<Bind> {
        self.result_rx.recv().unwrap()
    }
}

impl Evaluator for Pool {
    fn enqueue(&mut self, job: Job) {
        Pool::enqueue(self, move || {
            match job.process() {
                Ok(bind) => Some(bind),
                Err(e) => {
                    println!("{}", e);
                    None
                },
            }
        });
    }

    fn dequeue(&mut self) -> Option<Bind> {
        Pool::dequeue(self)
    }
}

impl Evaluator for VecDeque<Job> {
    fn enqueue(&mut self, job: Job) {
        self.push_back(job);
    }
    fn dequeue(&mut self) -> Option<Bind> {
        self.pop_front().and_then(|job| {
            match job.process() {
                Ok(bind) => Some(bind),
                Err(e) => {
                    println!("{}", e);
                    None
                },
            }
        })
    }
}
