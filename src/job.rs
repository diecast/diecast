use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Sender, SendError, Receiver, RecvError};
use std::collections::{BTreeMap, VecDeque, HashMap};
use std::thread;
use std::mem;
use std::fmt;

use binding::{self, Bind};
use dependency::Graph;
use rule::Rule;
use compiler;

pub struct Job {
    pub bind: Bind,
    pub compiler: Arc<Box<binding::Handler + Sync + Send>>,
}

impl fmt::Debug for Job {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{}]", self.bind.data.read().unwrap().name)
    }
}

impl Job {
    pub fn new<C>(bind: Bind, compiler: C) -> Job
    where C: binding::Handler + Sync + Send + 'static {
        Job {
            bind: bind,
            compiler: Arc::new(Box::new(compiler)),
        }
    }

    pub fn into_bind(self) -> Bind {
        self.bind
    }

    pub fn process(&mut self) -> compiler::Result {
        // <Compiler as binding::Handler>::handle(&self.compiler, &mut self.bind)
        self.compiler.handle(&mut self.bind)
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
        for _ in 0 .. threads {
            let rx = rx.clone();
            let tx = tx.clone();

            thread::spawn(move || {
                let sentinel = Sentinel::new(tx.clone());

                loop {
                    let message = {
                        // Only lock jobs for the time it takes
                        // to get a job, not run it.
                        let lock = rx.lock().unwrap();
                        lock.recv()
                    };

                    match message {
                        Ok(mut job) => {
                            trace!("dequeued {:?}", job);

                            match job.process() {
                                Ok(()) => {
                                    tx.send(Ok(job)).unwrap()
                                },
                                Err(e) => {
                                    println!("\nthe following job encountered an error:\n  {:?}\n\n{}\n", job, e);
                                    tx.send(Err(Error::Err)).unwrap();
                                }
                            }
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
    graph: Graph<String>,

    /// the dependency count of each binding
    dependencies: BTreeMap<String, usize>,

    /// a map of bindings to the list of jobs that haven't been processed yet
    waiting: VecDeque<Job>,

    /// finished dependencies
    finished: BTreeMap<String, Arc<Bind>>,

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
            dependencies: BTreeMap::new(),
            waiting: VecDeque::new(),
            finished: BTreeMap::new(),
            pool: Pool::new(threads),
            count: 0,
        }
    }

    pub fn add(&mut self, bind: Bind, rule: &Rule) {
        let binding = bind.data.read().unwrap().name.clone();

        // TODO: this still necessary?
        // it's only used to determine if anything will actually be done
        // operate on a binding-level
        self.count += 1;

        let compiler = rule.get_compiler();

        // if there's no compiler then no need to dispatch a job
        // or anything like that
        if let &Some(ref compiler) = compiler {
            self.waiting.push_front(Job::new(bind, compiler.clone()));
        }

        self.graph.add_node(binding.clone());

        for dep in rule.dependencies() {
            trace!("setting dependency {} -> {}", dep, binding);
            self.graph.add_edge(dep.clone(), binding.clone());
        }

        self.satisfy(&binding);
        self.enqueue_ready();
    }

    // TODO: will need Borrow bound
    fn satisfy(&mut self, binding: &str) {
        if let Some(dependents) = self.graph.dependents_of(binding) {
            let names = self.dependencies.keys().cloned().collect::<Vec<String>>();

            for name in names {
                if dependents.contains(&name) {
                    *self.dependencies.entry(name).get().unwrap_or_else(|v| v.insert(0)) -= 1;
                }
            }
        }
    }

    fn ready(&mut self) -> VecDeque<Job> {
        let waiting = mem::replace(&mut self.waiting, VecDeque::new());

        let (ready, waiting): (VecDeque<Job>, VecDeque<Job>) =
            waiting.into_iter()
               .partition(|job| self.dependencies[&job.bind.data.read().unwrap().name] == 0);

        self.waiting = waiting;

        trace!("the remaining order is {:?}", self.waiting);
        trace!("the ready bindings are {:?}", ready);

        ready
    }

    pub fn sort_jobs(&mut self, order: VecDeque<String>) {
        let mut job_map =
            mem::replace(&mut self.waiting, VecDeque::new())
            .into_iter()
            .map(|job| {
                let name = job.bind.data.read().unwrap().name.to_string();
                (name, job)
            })
            .collect::<HashMap<String, Job>>();

        // put the jobs into the order provided
        let ordered =
            order.into_iter()
            .map(|name| {
                let job = job_map.remove(&name).unwrap();

                // set dep counts
                let name = job.bind.data.read().unwrap().name.clone();
                let empty =
                    Arc::new(
                        Bind::new(
                            name.clone(),
                            job.bind.data.read().unwrap().configuration.clone()));

                let count = self.graph.dependency_count(&name);
                trace!("{} has {} dependencies", name, count);

                *self.dependencies.entry(name.clone()).get().unwrap_or_else(|v| v.insert(0)) += count;

                if job.bind.items.is_empty() {
                    trace!("{} is an empty dependency", name);
                    self.finished.insert(name.clone(), empty.clone());

                    // this can go negative but it balances out once the dep
                    // is processed by the above
                    trace!("decrementing dep count of dependents");
                    self.satisfy(&name);
                }

                return job;
            })
        .collect::<VecDeque<Job>>();

        mem::replace(&mut self.waiting, ordered);

        assert!(job_map.is_empty(), "not all jobs were sorted!");
    }

    pub fn execute(&mut self) {
        if self.count == 0 {
            println!("there is nothing to do");
            return;
        }

        match self.graph.resolve() {
            Ok(order) => {
                self.sort_jobs(order);

                println!("jobs: {:?}", self.waiting);

                trace!("enqueueing ready jobs");
                self.enqueue_ready();

                trace!("looping");
                loop {
                    match self.pool.dequeue().unwrap() {
                        Ok(job) => {
                            trace!("received job from pool");
                            if self.handle_done(job) {
                                break;
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
        self.waiting.clear();
        self.finished.clear();
        self.count = 0;
    }

    fn handle_done(&mut self, current: Job) -> bool {
        trace!("finished {}", current.bind.data.read().unwrap().name);
        trace!("before waiting: {:?}", self.waiting);

        if self.waiting.is_empty() {
            return true;
        }

        let binding = current.bind.data.read().unwrap().name.to_string();

        // binding is complete
        trace!("binding {} finished", binding);

        // if they're done, move from staging to finished
        self.finished.insert(binding.clone(), Arc::new({
            current.into_bind()
        }));

        if compiler.is_none() {
            self.satisfy(&binding);
            self.enqueue_ready();
        }

        return false;
    }

    // TODO: I think this should be part of satisfy
    // one of the benefits of keeping it separate is that
    // we can satisfy multiple bindings at once and then perform
    // a bulk enqueue_ready
    fn enqueue_ready(&mut self) {
        // prepare dependencies for now-ready dependents
        let mut deps_cache = BTreeMap::new();

        for mut job in self.ready() {
            if job.bind.items.is_empty() {
                continue;
            }

            let name = job.bind.data.read().unwrap().name.clone();
            trace!("{} is ready", name);

            deps_cache.entry(name.clone()).get().unwrap_or_else(|entry| {
                let mut deps = BTreeMap::new();

                // use Borrow?
                if let Some(ds) = self.graph.dependencies_of(&name) {
                    for dep in ds {
                        trace!("adding dependency: {:?}", dep);
                        deps.insert(dep.clone(), self.finished[dep].clone());
                    }
                }

                entry.insert(Arc::new(deps))
            });

            let deps = deps_cache[&name].clone();
            job.bind.set_dependencies(deps);
            trace!("job now ready: {:?}", job);
            self.pool.enqueue(job).unwrap();
        }
    }
}

