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

    /// TODO: probably make this a flat vec?
    /// a queue of jobs that are ready
    ///
    /// while let Some(jobs) = manager.ready() {
    ///     for job in jobs {
    ///         pool.enqueue(jobs)
    ///     }
    /// }
    ready: VecDeque<Vec<Job>>,

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
            ready: VecDeque::new(),
            staging: BTreeMap::new(),
            finished: BTreeMap::new(),
            paused: BTreeMap::new(),
            pool: Pool::new(threads),
            count: 0,
        }
    }

    pub fn add_job(&mut self,
           binding: &'static str,
           item: Item,
           compiler: Arc<Box<Compile>>,
           dependencies: &[&'static str]) {
        self.count += 1;

        let job = Job::new(binding, item, compiler, self.count);

        self.waiting.entry(binding).get().unwrap_or_else(|v| v.insert(vec![])).push(job);
        *self.item_count.entry(binding).get().unwrap_or_else(|v| v.insert(0)) += 1;

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

    fn satisfy(&mut self, binding: &'static str) {
        let dependents = self.graph.dependents_of(binding);

        if dependents.is_none() {
            return;
        }

        let dependents = dependents.unwrap();

        for name in self.waiting.keys() {
            if dependents.contains(name) {
                let count = self.dependencies.get_mut(name).unwrap();
                *count -= 1;
            }
        }
    }

    fn ready<'a>(&'a mut self) -> Drain<'a, Vec<Job>> {
        let order = mem::replace(&mut self.order, VecDeque::new());
        let (ready, waiting): (VecDeque<&'static str>, VecDeque<&'static str>) =
            order.into_iter().partition(|&binding| self.dependencies[binding] == 0);

        self.order = waiting;

        for name in ready {
            self.ready.push_back(self.waiting.remove(name).unwrap());
        }

        self.ready.drain()
    }

    pub fn execute(&mut self) {
        if self.count == 0 {
            println!("there is nothing to do");
            return;
        }

        match self.graph.resolve() {
            Ok(order) => {
                // swap out the jobs in order to consume them
                // satisfy dependents of empty rules
                // 1. iterate through bindings to find empty vecs
                // 2. create arc<empty vec> for key of #1
                // 3. iterate through dependents of #1
                // 4. insert #2 into deps of #3
                // 5. decrement dep count

                let mut jobs = mem::replace(&mut self.waiting, BTreeMap::new());

                println!("jobs: {:?}", jobs);

                let empty_deps: Arc<Vec<Item>> = Arc::new(Vec::new());
                // TODO: this requires that we always insert at least a name for job map?
                let empty_map =
                    jobs.iter()
                    .filter(|&(_, jobs)| jobs.is_empty())
                    .map(|(&name, _)| {
                        let mut bt = BTreeMap::new();
                        bt.insert(name, empty_deps.clone());

                        // TODO: hack to put it here?
                        self.finished.insert(name, empty_deps.clone());

                        (name, Arc::new(bt))
                    })
                    .collect::<BTreeMap<&'static str, ::item::Dependencies>>();

                let empty_binds = empty_map.keys().cloned().collect::<BTreeSet<&'static str>>();

                // set the dependency counts
                for (name, jobs) in &mut jobs {
                    let mut dep_count = self.graph.dependency_count(name);

                    let empty_dep = if let Some(deps) = self.graph.dependencies_of(name) {
                        let mut empty_deps = deps.intersection(&empty_binds).peekable();
                        let has_empty_deps = empty_deps.peek().is_some();

                        if dep_count == 1 && has_empty_deps {
                            dep_count -= 1;
                            Some(empty_map[*empty_deps.next().unwrap()].clone())
                        } else if has_empty_deps {
                            dep_count -= empty_deps.count();
                            None
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    self.dependencies.insert(name, dep_count);

                    if let Some(ref dep) = empty_dep {
                        for job in jobs {
                            job.item.dependencies = dep.clone();
                        }
                    }
                }

                // for jobs in self.manager.ready() {
                //     for job in jobs {
                //         self.pool.enqueue(job).unwrap();
                //     }
                // }

                for name in order {
                    if self.dependencies[name] == 0 {
                        if let Some(jobs) = jobs.remove(name) {
                            for job in jobs {
                                self.pool.enqueue(job).unwrap();
                            }
                        }
                    }
                }

                loop {
                    match self.pool.dequeue().unwrap() {
                        Ok(job) => {
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
        self.ready.clear();
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

        let dependents = self.graph.dependents_of(binding);

        // no dependents
        if dependents.is_none() {
            return false;
        }

        // prepare dependencies for now-ready dependents

        let dependents = dependents.unwrap();

        let mut ready = Vec::new();

        for name in self.item_count.keys() {
            if dependents.contains(name) {
                let count = self.dependencies.get_mut(name).unwrap();
                *count -= 1;

                // TODO: just check if it's 1
                if *count == 0 {
                    for job in self.waiting.remove(name).unwrap() {
                        // self.job_pool.enqueue(job).unwrap();
                        ready.push(job);
                    }
                }
            }
        }

        trace!("finished: {:?}", self.finished);

        let mut deps_cache = BTreeMap::new();
        let mut dependents = ready.iter().map(|j| j.binding).collect::<Vec<&'static str>>();

        dependents.sort();
        dependents.dedup();

        // this creates a cache of binding -> dependencies
        // this way multiple jobs that are part of the same binding
        // dont end up reconstructing the dependencies each time
        //
        // there's no need to add the save states due to barriers here,
        // because this will be the first time the jobs are going to run,
        // so they won't have reached any barriers to begin with
        for dependent in dependents {
            let mut deps = BTreeMap::new();

            for &dep in self.graph.dependencies_of(dependent).unwrap() {
                trace!("adding dependency: {:?}", dep);
                deps.insert(dep, self.finished[dep].clone());
            }

            deps_cache.insert(dependent, Arc::new(deps));
        }

        // attach the dependencies and dispatch
        for mut job in ready {
            job.item.dependencies = deps_cache[job.binding].clone();
            trace!("job now ready: {:?}", job);
            self.pool.enqueue(job).unwrap();
        }

        return false;
    }

}

