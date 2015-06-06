use std::sync::Arc;
use std::path::{PathBuf, Path};
use std::collections::{BTreeMap, BTreeSet, VecDeque, HashMap, HashSet};
use std::mem;

use configuration::Configuration;
use dependency::Graph;
use rule::{self, Rule};
use bind::{self, Bind};
use super::evaluator::Evaluator;
use super::Job;

pub struct Manager<E>
where E: Evaluator {
    configuration: Arc<Configuration>,

    rules: HashMap<String, Arc<Rule>>,

    graph: Graph<String>,

    /// Dependency count of each bind
    dependencies: BTreeMap<String, usize>,

    /// Map of binds to the list of jobs that haven't been processed yet
    waiting: Vec<Job>,

    /// Finished dependencies
    finished: BTreeMap<String, Arc<Bind>>,

    /// Thread pool to process jobs
    evaluator: E,

    // TODO
    // feels weird to have this here, but it's in-line with making
    // matching Patterns first-class
    /// Paths being considered
    paths: Arc<Vec<PathBuf>>,
}

impl<E> Manager<E>
where E: Evaluator {
    pub fn new(evaluator: E, configuration: Arc<Configuration>) -> Manager<E> {
        Manager {
            configuration: configuration,
            rules: HashMap::new(),
            graph: Graph::new(),
            dependencies: BTreeMap::new(),
            waiting: Vec::new(),
            finished: BTreeMap::new(),
            evaluator: evaluator,
            paths: Arc::new(Vec::new()),
        }
    }

    /// Re-enumerate the paths in the input directory
    pub fn update_paths(&mut self) {
        use walker::Walker;

        self.paths = Arc::new({
            Walker::new(&self.configuration.input).unwrap()
            .filter_map(|p| {
                let path = p.unwrap().path();

                if let Some(ref ignore) = self.configuration.ignore {
                    if ignore.matches(&Path::new(path.file_name().unwrap())) {
                        return None;
                    }
                }

                ::std::fs::metadata(&path)
                .map(|m|
                     if m.is_file() { Some(path.to_path_buf()) }
                     else { None }
                 ).unwrap_or(None)
            })
            .collect()
        });
    }

    pub fn add(&mut self, rule: Arc<Rule>) {
        // prepare bind-data with the name and configuration
        let data = bind::Data::new(String::from(rule.name()), self.configuration.clone());
        let name = data.name.clone();

        // TODO
        // instead of rule_count == 0,
        // check if self.waiting.is_empty()?

        // construct job from bind-data, rule kind, rule handler, and paths
        // push it to waiting queue
        self.waiting.push(
            Job::new(
                data,
                rule.kind().clone(),
                rule.handler().clone(),
                self.paths.clone()));

        self.graph.add_node(name.clone());

        // make its dependencies depend on this binding
        for dep in rule.dependencies() {
            self.graph.add_edge(dep.clone(), name.clone());
        }

        self.rules.insert(String::from(rule.name()), rule);
    }

    // TODO: will need Borrow bound
    fn satisfy(&mut self, bind: &str) {
        if let Some(dependents) = self.graph.dependents_of(bind) {
            let names = self.dependencies.keys().cloned().collect::<Vec<String>>();

            for name in names {
                if dependents.contains(&name) {
                    // FIXME this or_insert is incorrect, since using 0
                    // and subtracting 1 will cause underflow
                    if let Some(count) = self.dependencies.get_mut(&name) {
                        *count -= 1;
                    } else {
                        panic!("dependency count for {} is not available!", name);
                    }
                }
            }
        }
    }

    fn ready(&mut self) -> Vec<Job> {
        let waiting = mem::replace(&mut self.waiting, Vec::new());

        let (ready, waiting): (Vec<Job>, Vec<Job>) =
            waiting.into_iter()
               .partition(|job| self.dependencies[&job.bind_data.name] == 0);

        self.waiting = waiting;

        ready
    }

    pub fn sort_jobs(&mut self, order: VecDeque<String>) {
        assert!(self.waiting.len() == order.len(), "`waiting` and `order` are not the same length");

        let mut job_map =
            mem::replace(&mut self.waiting, Vec::new())
            .into_iter()
            .map(|job| {
                let name = job.bind_data.name.clone();
                (name, job)
            })
            .collect::<HashMap<String, Job>>();

        // put the jobs into the order provided
        let ordered =
            order.into_iter()
            .map(|name| {
                let job = job_map.remove(&name).unwrap();

                // set dep counts
                let name = job.bind_data.name.clone();

                let count = self.graph.dependency_count(&name);

                *self.dependencies.entry(name).or_insert(0) += count;

                return job;
            })
            .collect::<Vec<Job>>();

        mem::replace(&mut self.waiting, ordered);

        assert!(job_map.is_empty(), "not all jobs were sorted!");
    }

    pub fn build(&mut self) -> ::Result {
        if self.waiting.is_empty() {
            println!("there is nothing to do");
            return Ok(());
        }

        let count = self.waiting.len();

        let order = try!(self.graph.resolve_all());

        self.sort_jobs(order);

        self.enqueue_ready();

        // TODO: should have some sort of timeout here
        for _ in (0 .. count) {
            match self.evaluator.dequeue() {
                Some(job) => {
                    self.handle_done(job);
                },
                None => {
                    return Err(From::from("a job panicked. stopping everything"));
                }
            }
        }

        self.reset();

        Ok(())
    }

    // TODO paths ref
    pub fn update(&mut self, paths: HashSet<PathBuf>) -> ::Result {
        if self.waiting.is_empty() {
            println!("there is nothing to do");
            return Ok(());
        }

        let mut matched = vec![];
        let mut didnt = BTreeSet::new();

        // TODO handle deletes and new files
        // * deletes: full build
        // * new files: add Item

        let mut binds = HashMap::new();

        // find the binds that contain the paths

        for bind in self.finished.values() {
            use item;

            let name = bind.name.clone();
            let rule = &self.rules[&name];
            let kind = rule.kind().clone();

            let pattern =
                if let rule::Kind::Matching(ref pattern) = *kind {
                    pattern
                } else {
                    continue
                };

            // Borrow<Path> for &PathBuf
            // impl<'a, T, R> Borrow<T> for &'a R where R: Borrow<T>;

            let mut affected =
                paths.iter()
                .filter(|p| pattern.matches(p))
                .cloned()
                .collect::<HashSet<PathBuf>>();

            let is_match = affected.len() > 0;

            // TODO
            // preferably don't clone, instead just modify it in place
            let mut modified: Bind = (**bind).clone();

            for item in modified.items_mut() {
                if item.route().reading().map(|p| affected.remove(p)).unwrap_or(false) {
                    item::set_stale(item, true);
                }
            }

            // paths that were added
            // if affected.len() > 0 {
            //     for path in affected {
            //         bind.push(path);
            //     }
            // }

            bind::set_stale(&mut modified, true);

            if is_match {
                binds.insert(name.clone(), modified);
                matched.push(name);
            } else {
                didnt.insert(name);
            }
        }

        // no binds matched the path; nothing to update
        if matched.is_empty() {
            println!("no matches");
            return Ok(());
        }

        self.waiting.clear();

        // the name of each bind
        let order = try!(self.graph.resolve(matched));

        // create a job for each bind in the order
        for name in &order {
            let bind = &self.finished[name];
            let rule = &self.rules[&bind.name];

            // TODO: need a way to get the existing bind's Arc<Data>
            // perhaps: Bind.to_job(&rule)
            let mut job = Job::new(
                // TODO this might differ from binds bind?
                bind::get_data(&bind).clone(),
                rule.kind().clone(),
                rule.handler().clone(),
                self.paths.clone());

            job.bind = binds.remove(name);

            self.waiting.push(job);
        }

        let order_names = order.iter().cloned().collect::<BTreeSet<_>>();

        let didnt = didnt.difference(&order_names).cloned().collect::<BTreeSet<_>>();

        self.sort_jobs(order);

        // binds that aren't in the returned order should be assumed
        // to have already been satisfied
        for name in &order_names {
            if let Some(deps) = self.graph.dependencies_of(name) {
                let affected = deps.intersection(&didnt).count();
                *self.dependencies.get_mut(name).unwrap() -= affected;
            }
        }

        let count = self.waiting.len();

        self.enqueue_ready();

        // TODO: should have some sort of timeout here
        // FIXME
        // can't do while waiting.is_empty() becuase it could
        // be momentarily empty before the rest get added
        for _ in (0 .. count) {
            match self.evaluator.dequeue() {
                Some(job) => {
                    self.handle_done(job);
                },
                None => {
                    return Err(From::from("a job panicked. stopping everything"));
                }
            }
        }

        self.reset();

        Ok(())
    }

    // TODO: audit
    fn reset(&mut self) {
        self.graph = Graph::new();
        self.waiting.clear();
    }

    fn handle_done(&mut self, current: Job) {
        let bind = current.bind_data.name.clone();

        // if they're done, move from staging to finished
        self.finished.insert(bind.clone(), Arc::new({
            let mut bind = current.into_bind();
            bind::set_stale(&mut bind, false);
            bind
        }));

        self.satisfy(&bind);
        self.enqueue_ready();
    }

    fn enqueue_ready(&mut self) {
        for mut job in self.ready() {
            let name = job.bind_data.name.clone();

            if let Some(deps) = self.graph.dependencies_of(&name) {
                // insert each dependency
                for dep in deps {
                    job.bind_data.dependencies.insert(dep.clone(), self.finished[dep].clone());
                }
            }

            self.evaluator.enqueue(job);
        }
    }
}

