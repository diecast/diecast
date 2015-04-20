use std::sync::Arc;
use std::collections::{BTreeMap, VecDeque, HashMap};
use std::mem;

use dependency::Graph;
use rule::Rule;
use binding::Bind;
use super::evaluator::Evaluator;
use super::Job;

pub struct Manager<E>
where E: Evaluator {
    graph: Graph<String>,

    /// the dependency count of each binding
    dependencies: BTreeMap<String, usize>,

    /// a map of bindings to the list of jobs that haven't been processed yet
    waiting: VecDeque<Job>,

    /// finished dependencies
    finished: BTreeMap<String, Arc<Bind>>,

    /// Thread pool to process jobs
    evaluator: E,

    /// number of jobs being managed
    count: usize,
}

/// sample api:
///   manager.add_rule(rule);
///   manager.build();
///
/// later:
///   manager.update_path(path);

impl<E> Manager<E>
where E: Evaluator {
    pub fn new(evaluator: E) -> Manager<E> {
        Manager {
            graph: Graph::new(),
            dependencies: BTreeMap::new(),
            waiting: VecDeque::new(),
            finished: BTreeMap::new(),
            // TODO: this is what needs to change afaik
            evaluator: evaluator,
            count: 0,
        }
    }

    pub fn add(&mut self, rule: &Rule, bind: Bind) {
        let binding = bind.data().name.clone();

        // TODO: this still necessary?
        // it's only used to determine if anything will actually be done
        // operate on a binding-level
        self.count += 1;

        // if there's no handler then no need to dispatch a job
        // or anything like that
        self.waiting.push_front(Job::new(bind, rule.get_handler().clone()));

        self.graph.add_node(binding.clone());

        for dep in rule.dependencies() {
            trace!("setting dependency {} -> {}", dep, binding);
            self.graph.add_edge(dep.clone(), binding.clone());
        }
    }

    // TODO: will need Borrow bound
    fn satisfy(&mut self, binding: &str) {
        if let Some(dependents) = self.graph.dependents_of(binding) {
            let names = self.dependencies.keys().cloned().collect::<Vec<String>>();

            for name in names {
                if dependents.contains(&name) {
                    *self.dependencies.entry(name).or_insert(0) -= 1;
                }
            }
        }
    }

    fn ready(&mut self) -> VecDeque<Job> {
        let waiting = mem::replace(&mut self.waiting, VecDeque::new());

        let (ready, waiting): (VecDeque<Job>, VecDeque<Job>) =
            waiting.into_iter()
               .partition(|job| self.dependencies[&job.bind.data().name] == 0);

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
                let name = job.bind.data().name.to_string();
                (name, job)
            })
            .collect::<HashMap<String, Job>>();

        // put the jobs into the order provided
        let ordered =
            order.into_iter()
            .map(|name| {
                let job = job_map.remove(&name).unwrap();

                // set dep counts
                let name = job.bind.data().name.clone();

                let count = self.graph.dependency_count(&name);
                trace!("{} has {} dependencies", name, count);

                *self.dependencies.entry(name.clone()).or_insert(0) += count;

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

                trace!("enqueueing ready jobs");
                self.enqueue_ready();

                // TODO: should have some sort of timeout here
                trace!("looping");
                for _ in (0 .. self.count) {
                    match self.evaluator.dequeue() {
                        Some(job) => {
                            trace!("received job from pool");
                            self.handle_done(job);
                        },
                        None => {
                            println!("a job panicked. stopping everything");
                            ::std::process::exit(1);
                        }
                    }
                }
            },
            Err(cycle) => {
                println!("a dependency cycle was detected: {:?}", cycle);
                ::std::process::exit(1);
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
        trace!("finished {}", current.bind.data().name);
        trace!("before waiting: {:?}", self.waiting);

        let binding = current.bind.data().name.to_string();

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
            let name = job.bind.data().name.clone();
            trace!("{} is ready", name);

            deps_cache.entry(name.clone()).or_insert_with(|| {
                let mut deps = BTreeMap::new();

                // use Borrow?
                if let Some(ds) = self.graph.dependencies_of(&name) {
                    for dep in ds {
                        trace!("adding dependency: {:?}", dep);
                        deps.insert(dep.clone(), self.finished[dep].clone());
                    }
                }

                Arc::new(deps)
            });

            let deps = deps_cache[&name].clone();

            job.bind = Bind::with_dependencies(job.bind, deps);

            trace!("job now ready: {:?}", job);

            self.evaluator.enqueue(job);
        }
    }
}

