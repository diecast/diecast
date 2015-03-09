use std::fmt;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Sender, SendError, Receiver, RecvError};
use std::thread;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::collections::btree_map::Entry::{Vacant, Occupied};
use std::collections::vec_deque::Drain;
use std::mem;

use compiler::{self, Compile, is_paused};
use item::Item;
use dependency::Graph;
use rule::Rule;

pub struct Job {
    pub id: usize,

    // TODO: not a fan of this being here. maybe global rwlock lookup-table?
    pub binding: &'static str,

    pub item: Item,
    pub compiler: Arc<Box<Compile>>,

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
        trace!("using {} threads", threads);

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

pub struct Manager {
    graph: Graph<&'static str>,

    /// TODO: should this probably also include the Path and other info?
    ///       or perhaps have reverse-map of path -> binding? idk
    /// number of items found
    item_count: BTreeMap<&'static str, usize>,

    /// the dependency count of each binding
    dependencies: BTreeMap<&'static str, usize>,

    /// the order that the bindings have to be evaluated in
    order: VecDeque<&'static str>,

    /// a map of bindings to the list of jobs that haven't been processed yet
    waiting: BTreeMap<&'static str, Vec<Job>>,

    /// the dependencies as they're being built
    staging: BTreeMap<&'static str, Vec<Item>>,

    /// finished dependencies
    finished: BTreeMap<&'static str, Arc<Vec<Item>>>,

    /// dependencies that are currently paused due to a barrier
    paused: BTreeMap<&'static str, Vec<Job>>,

    /// Thread pool to process jobs
    pool: Pool,

    /// number of jobs being managed
    count: usize,
}

/// sample api:
///   manager.add_rule(rule);
///   manager.build();
///
/// later:
///   manager.update_path(path);

impl Manager {
    pub fn new(threads: usize) -> Manager {
        Manager {
            graph: Graph::new(),
            item_count: BTreeMap::new(),
            dependencies: BTreeMap::new(),
            order: VecDeque::new(),
            waiting: BTreeMap::new(),
            staging: BTreeMap::new(),
            finished: BTreeMap::new(),
            paused: BTreeMap::new(),
            pool: Pool::new(threads),
            count: 0,
        }
    }

    pub fn add(&mut self,
               binding: &'static str,
               compiler: Arc<Box<Compile>>,
               dependencies: &[&'static str],
               items: Vec<Item>) {
        self.waiting.insert(binding, vec![]);
        self.item_count.insert(binding, items.len());

        for item in items {
            self.count += 1;

            let job =
                Job::new(
                    binding,
                    item,
                    compiler.clone(),
                    self.count);

            self.waiting.get_mut(binding).unwrap().push(job);

            // TODO: this will be set to the correct number after graph resolution?
            self.dependencies.insert(binding, 0);

            self.graph.add_node(binding);

            if !dependencies.is_empty() {
                trace!("has dependencies: {:?}", dependencies);

                for &dep in dependencies {
                    trace!("setting dependency {} -> {}", dep, binding);
                    self.graph.add_edge(dep, binding);
                }
            }
        }
    }

    fn satisfy(&mut self, binding: &'static str) {
        if let Some(dependents) = self.graph.dependents_of(binding) {
            for name in self.item_count.keys() {
                if dependents.contains(name) {
                    *self.dependencies.entry(name).get().unwrap_or_else(|v| v.insert(0)) -= 1;
                }
            }
        }
    }

    fn ready(&mut self) -> VecDeque<(&'static str, Vec<Job>)> {
        let order = mem::replace(&mut self.order, VecDeque::new());
        let (ready, waiting): (VecDeque<&'static str>, VecDeque<&'static str>) =
            order.into_iter().partition(|&binding| self.dependencies[binding] == 0);

        self.order = waiting;

        trace!("the remaining order is {:?}", self.order);
        trace!("the ready bindings are {:?}", ready);

        ready.iter().map(|&name| (name, self.waiting.remove(name).unwrap())).collect()
    }

    pub fn execute(&mut self) {
        if self.count == 0 {
            println!("there is nothing to do");
            return;
        }

        match self.graph.resolve() {
            Ok(order) => {
                self.order = order;

                println!("jobs: {:?}", self.waiting);

                trace!("setting dependency counts");
                self.set_dependency_counts();

                trace!("enqueueing ready jobs");
                self.enqueue_ready();

                trace!("looping");
                loop {
                    match self.pool.dequeue().unwrap() {
                        Ok(job) => {
                            trace!("received job from pool");
                            if job.is_paused {
                                self.handle_paused(job);
                            } else {
                                if self.handle_done(job) {
                                    break;
                                }
                            }
                        },
                        Err(Error::Err) => {
                            println!("a job returned an error. stopping everything");
                            ::exit(1);
                        },
                        Err(Error::Panic) => {
                            println!("a job panicked. stopping everything");
                            ::exit(1);
                        }
                    }
                }
            },
            Err(cycle) => {
                println!("a dependency cycle was detected: {:?}", cycle);
                ::exit(1);
            },
        }

        self.reset();
    }

    // TODO: audit
    fn reset(&mut self) {
        self.graph = Graph::new();
        self.item_count.clear();
        self.order.clear();
        self.waiting.clear();
        self.staging.clear();
        self.finished.clear();
        self.paused.clear();
        self.count = 0;
    }

    fn handle_paused(&mut self, current: Job) {
        trace!("paused {}", current.id);

        let binding = current.binding;

        // FIXME: this should use job.item.data.get::<Barriers>().unwrap().counts.last()
        // total number of jobs in the binding
        // let total = self.bindings[binding].len();
        let total =
            current.item.data.get::<compiler::Barriers>()
                .and_then(|bars| {
                    let counts = bars.counts.lock().unwrap();
                    counts.last().cloned()
                })
                .unwrap_or_else(|| self.item_count[binding]);

        println!("barrier limit for {} is {}", current.item, total);

        // add this paused job to the collection of
        // paused jobs for this binding
        // and return whether all jobs have been paused
        let finished = {
            let jobs = self.paused.entry(binding).get().unwrap_or_else(|v| v.insert(vec![]));
            jobs.push(current);
            jobs.len() == total
        };

        trace!("paused so far ({}): {:?}", self.paused[binding].len(), self.paused[binding]);
        trace!("total to pause: {}", total);
        trace!("finished: {}", finished);

        // there are still more jobs that haven't hit the barrier
        if !finished {
            return;
        }

        // get paused jobs to begin re-dispatching them
        let jobs = self.paused.remove(&binding).unwrap();

        // create the new set of dependencies
        // if the binding already had dependencies, copy them
        let mut new_deps = (*jobs[0].item.dependencies).clone();

        // insert the frozen state of these jobs
        new_deps.insert(binding, {
            Arc::new(jobs.iter()
                .map(|j| j.item.clone())
                .collect::<Vec<Item>>())
        });

        trace!("checking dependencies of \"{}\"", binding);

        let arc_map = Arc::new(new_deps);

        // package each job with its dependencies
        for mut job in jobs {
            job.item.dependencies = arc_map.clone();

            trace!("re-enqueuing: {:?}", job);

            self.pool.enqueue(job).unwrap();
        }
    }

    fn handle_done(&mut self, current: Job) -> bool {
        trace!("finished {}", current.id);
        trace!("before waiting: {:?}", self.waiting);

        if self.waiting.is_empty() {
            return true;
        }

        let binding = current.binding;
        let total = self.item_count[binding];

        // add to collection of finished items
        let finished = match self.staging.entry(binding) {
            Vacant(entry) => {
                entry.insert(vec![current.item]);
                1 == total
            },
            Occupied(mut entry) => {
                entry.get_mut().push(current.item);
                entry.get().len() == total
            },
        };

        trace!("checking if done");

        // if the binding isn't complete, nothing more to do
        if !finished {
            return false;
        }

        // binding is complete
        trace!("binding {} finished", binding);

        // if they're done, move from staging to finished
        self.finished.insert(binding, Arc::new({
            self.staging.remove(&binding).unwrap()
        }));

        self.satisfy(binding);
        self.enqueue_ready();

        return false;
    }

    fn set_dependency_counts(&mut self) {
        let empty = Arc::new(Vec::new());

        let mut waiting = mem::replace(&mut self.waiting, BTreeMap::new());

        // TODO: consolidate with satisfy()?
        // loop again because now all dependencies are set
        for (name, jobs) in &waiting {
            let count = self.graph.dependency_count(name);
            trace!("{} has {} dependencies", name, count);

            // this contortion allows us to use a single loop instead of looping once
            // to set the dependencies then looping again to do all of this
            // the way this works is that the current binding gets its dependency count
            // added to a potentially-existing dependency count
            // this way, the dependency count can go negative due to the code below,
            // but once that binding gets run through this it will add the dependency
            // count to it, essentially having the same effect
            *self.dependencies.entry(name).get().unwrap_or_else(|v| v.insert(0)) += count;

            trace!("item count of {}: {:?}", name, self.item_count.get(name).cloned());

            if jobs.is_empty() {
                trace!("{} is an empty dependency", name);
                self.finished.insert(name, empty.clone());

                trace!("decrementing dep count of dependents");
                self.satisfy(name);
            }
        }

        self.waiting = mem::replace(&mut waiting, BTreeMap::new());
    }

    fn enqueue_ready(&mut self) {
        // prepare dependencies for now-ready dependents
        let mut deps_cache = BTreeMap::new();

        for (name, jobs) in self.ready() {
            if jobs.is_empty() {
                continue;
            }

            trace!("{} is ready", name);

            if let Vacant(entry) = deps_cache.entry(name) {
                let mut deps = BTreeMap::new();

                if let Some(ds) = self.graph.dependencies_of(name) {
                    for &dep in ds {
                        trace!("adding dependency: {:?}", dep);
                        deps.insert(dep, self.finished[dep].clone());
                    }
                }

                entry.insert(Arc::new(deps));
            }

            for mut job in jobs {
                job.item.dependencies = deps_cache[job.binding].clone();
                trace!("job now ready: {:?}", job);
                self.pool.enqueue(job).unwrap();
            }
        }
    }
}

