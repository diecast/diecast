//! Site generation.

use std::sync::{Arc, TaskPool};
use std::sync::mpsc::{Sender, Receiver, channel};
use std::collections::{BTreeMap, RingBuf};
use std::collections::btree_map::Entry::{Vacant, Occupied};
use std::fs;

use pattern::Pattern;
use job::Job;
use compiler::Compiler;
use compiler::Status::{Paused, Done};
use item::Item;
use dependency::Graph;
use configuration::Configuration;
use rule::{self, Rule};

use std::path::{PathBuf, Path};

/// A Site scans the input path to find
/// files that match the given pattern. It then
/// takes each of those files and passes it through
/// the compiler chain.
pub struct Site {
    configuration: Arc<Configuration>,

    /// Mapping the id to its dependencies
    bindings: BTreeMap<&'static str, Vec<usize>>,

    /// The jobs
    jobs: Vec<Job>,

    /// Dependency resolution
    graph: Graph<&'static str>,

    /// Thread pool to process jobs
    thread_pool: TaskPool,

    /// For worker threads to send result back to main
    result_tx: Sender<Job>,

    /// For main thread to receive results
    result_rx: Receiver<Job>,

    /// the dependencies as they're being built
    staging_deps: BTreeMap<&'static str, Vec<Item>>,

    /// finished dependencies
    finished_deps: BTreeMap<&'static str, Arc<Vec<Item>>>,

    /// dependencies that are currently paused due to a barrier
    paused: BTreeMap<&'static str, Vec<Job>>,

    /// items whose dependencies have not been fulfilled
    waiting: Vec<Job>,

    rules: Vec<Rule>,
}

impl Site {
    pub fn new(configuration: Configuration) -> Site {
        let threads = configuration.threads;

        trace!("output directory is: {:?}", configuration.output);
        trace!("using {} threads", threads);

        // channels for sending and receiving results
        let (result_tx, result_rx) = channel();

        Site {
            configuration: Arc::new(configuration),

            bindings: BTreeMap::new(),
            jobs: Vec::new(),
            graph: Graph::new(),

            thread_pool: TaskPool::new(threads),
            result_tx: result_tx,
            result_rx: result_rx,

            staging_deps: BTreeMap::new(),
            finished_deps: BTreeMap::new(),
            paused: BTreeMap::new(),

            waiting: Vec::new(),
            rules: Vec::new(),
        }
    }
}

// TODO: audit this
fn sort_jobs(jobs: &mut Vec<Job>, order: &RingBuf<&'static str>, graph: &Graph<&'static str>) {
    let mut boundary = 0;

    for &bind_id in order {
        let mut idx = boundary;

        while idx < jobs.len() {
            let current_binding = jobs[idx].binding;

            if current_binding == bind_id {
                jobs[idx].dependency_count = graph.dependency_count(current_binding);
                jobs.swap(boundary, idx);
                boundary += 1;
            }

            idx += 1;
        }
    }

}

impl Site {
    fn dispatch_job(&self, mut job: Job) {
        let result_tx = self.result_tx.clone();

        self.thread_pool.execute(move || {
            job.process();
            result_tx.send(job).unwrap();
        });
    }

    fn handle_paused(&mut self, current: Job) {
        trace!("paused {}", current.id);

        let binding = current.binding;

        // total number of jobs in the binding
        let total = self.bindings[binding].len();

        // add this paused job to the collection of
        // paused jobs for this binding
        // and return whether all jobs have been paused
        let finished = {
            let jobs = self.paused.entry(binding).get()
                           .unwrap_or_else(|v| v.insert(vec![]));
            jobs.push(current);
            jobs.len() == total
        };

        trace!("paused so far ({}): {:?}",
               self.paused[binding].len(),
               self.paused[binding]);
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

            self.dispatch_job(job);
        }
    }

    fn handle_done(&mut self, current: Job) -> bool {
        trace!("finished {}", current.id);
        trace!("before waiting: {:?}", self.waiting);

        if self.waiting.is_empty() {
            return true;
        }

        let binding = current.binding;
        let total = self.bindings[binding].len();

        // add to collection of finished items
        let finished = match self.staging_deps.entry(binding) {
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
        self.finished_deps.insert(binding, Arc::new({
            self.staging_deps.remove(&binding).unwrap()
        }));

        let dependents = self.graph.dependents_of(binding);

        // no dependents
        if dependents.is_none() {
            return false;
        }

        // prepare dependencies for now-ready dependents

        let dependents = dependents.unwrap();

        // decrement the dependency count of dependents in the waiting queue
        for job in &mut self.waiting {
            if dependents.contains(&job.binding) {
                job.dependency_count -= 1;
            }
        }

        trace!("finished_deps: {:?}", self.finished_deps);

        // swap out in order to partition
        let waiting = ::std::mem::replace(&mut self.waiting, Vec::new());

        // separate out the now-ready jobs
        let (ready, waiting): (Vec<Job>, Vec<Job>) =
            waiting.into_iter().partition(|ref job| job.dependency_count == 0);

        self.waiting = waiting;

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
                deps.insert(dep, self.finished_deps[dep].clone());
            }

            deps_cache.insert(dependent, Arc::new(deps));
        }

        // attach the dependencies and dispatch
        for mut job in ready {
            job.item.dependencies = deps_cache[job.binding].clone();
            trace!("job now ready: {:?}", job);
            self.dispatch_job(job)
        }

        return false;
    }

    pub fn build(&mut self) {
        use std::fs::PathExt;
        use std::mem;

        // TODO: clean out the output directory here to avoid cruft and conflicts

        let rules = mem::replace(&mut self.rules, Vec::new());

        // TODO: the bindings should be performed in-order, since they
        // might depend on dependencies that have not been registered yet
        // causing errors
        //
        // the problem with this is that it ends up requiring to scan through
        // the entire paths multiple times since a create order might be interleaved
        // with matching orders :/
        //
        // perhaps allow orders to be placed in any order, but require that they be
        // ordered by dependencies whenever it's necessary? this would require some over
        // head though
        //
        // what would this entail? it seems like this would introduce dependency resolution
        // before dependency resolution is even run?
        //
        // NOTE: if any order is possible, then we can't enforce that dependencies be registered
        // 'by-reference' to avoid string errors
        //
        // * first process the items with no dependencies?

        // TODO: separate creating and matching collections? to avoid
        //       looping twice
        let paths =
            fs::walk_dir(&self.configuration.input).unwrap()
            .filter_map(|p| {
                let path = p.unwrap().path();

                if let Some(ref pattern) = self.configuration.ignore {
                    if pattern.matches(&Path::new(path.file_name().unwrap().to_str().unwrap())) {
                        return None;
                    }
                }

                if path.is_file() {
                    Some(path.to_path_buf())
                } else {
                    None
                }
            })
            .collect::<Vec<PathBuf>>();

        for rule in &rules {
            match rule.kind {
                rule::Kind::Creating(ref path) => {
                    let compiler = rule.compiler.clone();
                    let conf = self.configuration.clone();

                    println!("creating: {:?}", path);

                    self.add_job(
                        rule.name,
                        Item::new(conf, None, Some(path.clone())),
                        compiler,
                        &rule.dependencies);
                },
                rule::Kind::Matching(ref pattern) => {
                    for path in &paths {
                        let relative =
                            path.relative_from(&self.configuration.input)
                            .unwrap()
                            .to_path_buf();

                        let conf = self.configuration.clone();

                        if pattern.matches(&relative) {
                            self.add_job(
                                rule.name,
                                // TODO: make Vec<PathBuf> -> Vec<Arc<PathBuf>> to avoid copying?
                                //       what does this mean?
                                Item::new(conf, Some(relative), None),
                                rule.compiler.clone(),
                                &rule.dependencies);
                        }
                    }
                },
            }
        }

        mem::replace(&mut self.rules, rules);

        match self.graph.resolve() {
            Ok(order) => {
                use std::mem;

                // create the output directory
                fs::create_dir_all(&self.configuration.output)
                    .unwrap();

                // put the jobs in the correct evaluation order
                sort_jobs(&mut self.jobs, &order, &self.graph);

                // swap out the jobs in order to consume them
                let ordered = mem::replace(&mut self.jobs, Vec::new());

                trace!("ordered: {:?}", ordered);

                // total number of jobs
                let total_jobs = ordered.len();

                // determine which jobs are ready
                let (ready, waiting): (Vec<Job>, Vec<Job>) =
                   ordered.into_iter().partition(|ref job| job.dependency_count == 0);

                self.waiting = waiting;

                trace!("jobs: {}", total_jobs);
                trace!("ready: {:?}", ready);

                // dispatch the jobs that are ready
                for job in ready {
                    self.dispatch_job(job);
                }

                // possible to use self.result_rx.iter()?
                loop {
                    let current = self.result_rx.recv().unwrap();

                    match current.compiler.status {
                        Paused => {
                            self.handle_paused(current);
                        },
                        Done => {
                            if self.handle_done(current) {
                                break;
                            }
                        },
                    }
                }
            },
            Err(cycle) => {
                panic!("a dependency cycle was detected: {:?}", cycle);
            },
        }
    }

    // TODO: ensure can only add dependency on &Dependency to avoid string errors
    fn add_job(&mut self,
               binding: &'static str,
               item: Item,
               compiler: Compiler,
               dependencies: &[&'static str]) {
        trace!("adding job for {:?}", item);

        // create a job id
        let index = self.jobs.len();
        trace!("index: {}", index);

        // add the job to the list of jobs
        self.jobs.push(Job::new(binding, item, compiler, index));

        // associate the job id with the binding
        self.bindings.entry(binding).get()
            .unwrap_or_else(|v| v.insert(vec![]))
            .push(index);

        trace!("bindings: {:?}", self.bindings);

        // add the binding to the graph
        self.graph.add_node(binding);

        // if there are dependencies, create an edge from the dependency to this binding
        if !dependencies.is_empty() {
            trace!("has dependencies: {:?}", dependencies);

            for &dep in dependencies {
                trace!("setting dependency {} -> {}", dep, binding);
                self.graph.add_edge(dep, binding);
            }
        }
    }

    pub fn bind(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    pub fn configuration(&self) -> Arc<Configuration> {
        self.configuration.clone()
    }
}

