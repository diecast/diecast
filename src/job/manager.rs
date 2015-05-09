use std::sync::Arc;
use std::path::PathBuf;
use std::collections::{BTreeMap, BTreeSet, VecDeque, HashMap, HashSet};
use std::mem;

use configuration::Configuration;
use dependency::Graph;
use rule::{self, Rule};
use binding::{self, Bind};
use super::evaluator::Evaluator;
use super::Job;

pub struct Manager<E>
where E: Evaluator {
    configuration: Arc<Configuration>,

    rules: HashMap<String, Arc<Rule>>,

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
    pub fn new(evaluator: E, configuration: Arc<Configuration>) -> Manager<E> {
        Manager {
            configuration: configuration,
            rules: HashMap::new(),
            graph: Graph::new(),
            dependencies: BTreeMap::new(),
            waiting: VecDeque::new(),
            finished: BTreeMap::new(),
            // TODO: this is what needs to change afaik
            evaluator: evaluator,
            count: 0,
        }
    }

    pub fn add(&mut self, rule: Arc<Rule>) {
        let bind = binding::Data::new(String::from(rule.name()), self.configuration.clone());
        let binding = bind.name.clone();

        // TODO: this still necessary?
        // it's only used to determine if anything will actually be done
        // operate on a binding-level
        self.count += 1;

        // if there's no handler then no need to dispatch a job
        // or anything like that
        self.waiting.push_front(Job::new(bind, rule.get_source().clone(), rule.get_handler().clone()));

        self.graph.add_node(binding.clone());

        for dep in rule.dependencies() {
            trace!("setting dependency {} -> {}", dep, binding);
            self.graph.add_edge(dep.clone(), binding.clone());
        }

        self.rules.insert(String::from(rule.name()), rule);
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
               .partition(|job| self.dependencies[&job.bind.name] == 0);

        self.waiting = waiting;

        trace!("the remaining order is {:?}", self.waiting);
        trace!("the ready bindings are {:?}", ready);

        ready
    }

    pub fn sort_jobs(&mut self, order: VecDeque<String>) {
        assert!(self.waiting.len() == order.len(), "`waiting` and `order` are not the same length");

        let mut job_map =
            mem::replace(&mut self.waiting, VecDeque::new())
            .into_iter()
            .map(|job| {
                let name = job.bind.name.clone();
                (name, job)
            })
            .collect::<HashMap<String, Job>>();

        // put the jobs into the order provided
        let ordered =
            order.into_iter()
            .map(|name| {
                let job = job_map.remove(&name).unwrap();

                // set dep counts
                let name = job.bind.name.clone();

                let count = self.graph.dependency_count(&name);
                trace!("{} has {} dependencies", name, count);

                *self.dependencies.entry(name).or_insert(0) += count;

                return job;
            })
            .collect::<VecDeque<Job>>();

        mem::replace(&mut self.waiting, ordered);

        assert!(job_map.is_empty(), "not all jobs were sorted!");
    }

    pub fn build(&mut self) {
        if self.count == 0 {
            println!("there is nothing to do");
            return;
        }

        match self.graph.resolve_all() {
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

    // TODO paths ref
    pub fn update(&mut self, paths: HashSet<PathBuf>) {
        if self.count == 0 {
            println!("there is nothing to do");
            return;
        }

        let mut matched = vec![];
        let mut didnt = BTreeSet::new();

        // TODO handle deletes? how? on delete, do full build no matter what

        let mut bindings = HashMap::new();

        // find the binds that contain the paths
        for bind in self.finished.values() {
            use item::{Item, Route};

            let name = bind.data().name.clone();
            let rule = &self.rules[&name];

            if let &rule::Kind::Create = rule.kind() {
                continue;
            }

            let bind_paths =
                rule.get_source().source(bind.get_data()).into_iter()
                .map(|i| i.route.reading().unwrap().to_path_buf())
                .collect::<HashSet<PathBuf>>();

            let affected =
                bind_paths.intersection(&paths).cloned()
                .map(|p| Item::new(Route::Read(p), bind.get_data()))
                .collect::<Vec<Item>>();
            let is_match = affected.len() > 0;

            let mut modified: Bind = (**bind).clone();

            // TODO
            // what does it mean if it's left as a Partial?
            // in subsequent builds, as a dependent of a different affected?
            // it seems to me like a Partial should only apply on a single iteration,
            // and then it should flip back to Full
            modified.update(affected);

            if is_match {
                bindings.insert(name.clone(), modified);
                matched.push(name);
            } else {
                didnt.insert(name);
            }
        }

        if matched.is_empty() {
            trace!("no binds matched the path");
            return;
        }

        self.waiting.clear();

        // the name of each binding
        match self.graph.resolve(matched) {
            Ok(order) => {
                // create a job for each bind in the order
                for name in &order {
                    let bind = &self.finished[name];
                    let rule = &self.rules[&bind.data().name];

                    let mut job = Job::new(
                        // TODO this might differ from bindings bind?
                        bind.data().clone(),
                        rule.get_source().clone(),
                        rule.get_handler().clone());

                    job.binding = bindings.remove(name);

                    self.waiting.push_front(job);
                }

                let order_names = order.clone();
                let job_count = order.len();

                self.sort_jobs(order);

                // binds that aren't in the returned order should be assumed
                // to have already been satisfied
                for name in &order_names {
                    if let Some(deps) = self.graph.dependencies_of(name) {
                        let affected = deps.intersection(&didnt).count();
                        *self.dependencies.get_mut(name).unwrap() -= affected;
                    }
                }

                trace!("enqueueing ready jobs");
                self.enqueue_ready();

                // TODO: should have some sort of timeout here
                // FIXME
                // can't do while waiting.is_empty() becuase it could
                // be momentarily empty before the rest get added
                trace!("looping");
                for _ in (0 .. job_count) {
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
        self.count = 0;
    }

    fn handle_done(&mut self, current: Job) {
        trace!("finished {}", current.bind.name);
        trace!("before waiting: {:?}", self.waiting);

        let binding = current.bind.name.clone();

        // binding is complete
        trace!("binding {} finished", binding);

        // if they're done, move from staging to finished
        self.finished.insert(binding.clone(), Arc::new({
            let mut bind = current.into_bind();
            bind.set_full_build();
            bind
        }));

        self.satisfy(&binding);
        self.enqueue_ready();
    }

    // TODO: I think this should be part of satisfy
    // one of the benefits of keeping it separate is that
    // we can satisfy multiple bindings at once and then perform
    // a bulk enqueue_ready
    fn enqueue_ready(&mut self) {
        for mut job in self.ready() {
            let name = job.bind.name.clone();
            trace!("{} is ready", name);

            // use Borrow?
            if let Some(ds) = self.graph.dependencies_of(&name) {
                for dep in ds {
                    trace!("adding dependency: {:?}", dep);
                    job.bind.dependencies.insert(dep.clone(), self.finished[dep].clone());
                }
            }

            trace!("job now ready: {:?}", job);

            self.evaluator.enqueue(job);
        }
    }
}

