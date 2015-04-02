use std::sync::Arc;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::collections::{BTreeMap, VecDeque, HashMap};
use std::mem;
use std::fmt;

use threadpool::ThreadPool;

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

    // Cancel and destroy this sentinel.
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
    /// Spawns a new thread pool with `threads` threads.
    ///
    /// # Panics
    ///
    /// This function will panic if `threads` is 0.
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

pub struct Manager {
    graph: Graph<String>,

    /// the dependency count of each binding
    dependencies: BTreeMap<String, usize>,

    /// a map of bindings to the list of jobs that haven't been processed yet
    waiting: VecDeque<Job>,

    /// finished dependencies
    finished: BTreeMap<String, Arc<Bind>>,

    /// Thread pool to process jobs
    pool: Pool<Job>,

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

        // TODO: is this necessary?
        if compiler.is_none() {
            self.satisfy(&binding);
            self.enqueue_ready();
        }
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

                let count = self.graph.dependency_count(&name);
                trace!("{} has {} dependencies", name, count);

                *self.dependencies.entry(name.clone()).get().unwrap_or_else(|v| v.insert(0)) += count;

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
                for _ in (0 .. self.count) {
                    match self.pool.dequeue() {
                        Some(job) => {
                            trace!("received job from pool");
                            self.handle_done(job);
                        },
                        None => {
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

    fn handle_done(&mut self, current: Job) {
        trace!("finished {}", current.bind.data.read().unwrap().name);
        trace!("before waiting: {:?}", self.waiting);

        let binding = current.bind.data.read().unwrap().name.to_string();

        // binding is complete
        trace!("binding {} finished", binding);

        // if they're done, move from staging to finished
        self.finished.insert(binding.clone(), Arc::new({
            current.into_bind()
        }));

        self.satisfy(&binding);
        self.enqueue_ready();
    }

    // TODO: I think this should be part of satisfy
    // one of the benefits of keeping it separate is that
    // we can satisfy multiple bindings at once and then perform
    // a bulk enqueue_ready
    fn enqueue_ready(&mut self) {
        // prepare dependencies for now-ready dependents
        let mut deps_cache = BTreeMap::new();

        for mut job in self.ready() {
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

            self.pool.enqueue(move || {
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
    }
}

