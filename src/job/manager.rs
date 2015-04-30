use std::sync::Arc;
use std::path::{Path, PathBuf};
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

        // TODO
        //
        // we should start by first rebuilding the entire bind
        // to do this we need to run the source of each bind to determine
        // if the source matches
        //
        // FIXME
        // build() should clean
        // update() should not clean

        // TODO
        // * CANDIDATES: subset of binds that are Kind::Read
        //
        // determine which CANDIDATE contains the path
        // if found: mark bind as Update(path)
        //           entails getting it from self.finished, marking it
        // else: run all CANDIDATE source() and see if path is contained
        //       if found: rebuild entire (?) matched bind. ?: for things like Adjacent
        //       else: ignore, since no bind would handle
        //
        // to get things ready for a rebuild, it's necessary to get everything
        // from self.finished and mapping it to a Job and finally inserting it
        // into self.waiting
        //
        // * deref should conditionally deref to either all items (incl. cached)
        //   or the updated one
        // * explicit unsafe method will give access to all items (e.g. items_mut())

        // NOTE
        // * should we make Kind::Read only operate based on Pattern,
        //   as was previously the case?
        //
        //   PROS
        //   * fast detection of which bind is responsible
        //
        //   CONS
        //   * not as flexible? I can't envision a situation where dynamic
        //     item creation would be useful
        //
        // * it's probably _very_ beneficial to be able to access a Rule
        //   that corresponds to a Bind, gives access to Kind
        //
        //   NOTE try to avoid that, because then the bind handlers
        //        would have access to it
        //
        // * cached binds are in self.finished

        // FIXME
        // * a single path may be read from multiple bindings
        //   resolve_from only performs this from a single binding?
        //   * potential solution: find common ancestor
        // * a Job no longer takes a Bind, it only takes a binding::Data
        //   so it's not possible to go from an existing Bind to a Job
        // * in particular the above is true because we can't get the
        //   binding.data arc; expose new method for this?

        let mut matched = vec![];
        let mut didnt = BTreeSet::new();

        for bind in self.finished.values() {
            let name = bind.data().name.clone();
            let rule = &self.rules[&name];

            if let &rule::Kind::Create = rule.kind() {
                continue;
            }

            // TODO FIXME optimization
            // save the results of the sourcing and prepare
            // the binding to include it?
            let is_match =
                rule.get_source().source(bind.get_data()).iter()
                .find(|&item| {
                    // https://github.com/rust-lang/rust/issues/24969
                    paths.contains(&item.source().unwrap())
                }).is_some();

            if is_match {
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
                for name in &order {
                    let bind = &self.finished[name];
                    let rule = &self.rules[&bind.data().name];

                    // TODO: redesign to accomodate this
                    self.waiting.push_front(
                        Job::new(
                            bind.data().clone(),
                            rule.get_source().clone(),
                            rule.get_handler().clone()));
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
                for i in (0 .. job_count) {
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
        // self.finished.clear();
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

