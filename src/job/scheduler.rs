use std::sync::Arc;
use std::path::{PathBuf, Path};
use std::collections::{BTreeMap, VecDeque, HashMap};
use std::mem;

use futures::{self, Future, BoxFuture};
use futures_cpupool::CpuPool;

use configuration::Configuration;
use dependency::Graph;
use rule::Rule;
use bind::{self, Bind};
use super::Job;

pub struct Scheduler {
    configuration: Arc<Configuration>,

    rules: HashMap<String, Arc<Rule>>,

    graph: Graph<String>,

    /// Dependency count of each bind
    dependencies: BTreeMap<String, usize>,

    /// List of jobs that haven't been processed yet
    waiting: Vec<Job>,

    /// List of jobs currently being processed
    pending: Vec<BoxFuture<Bind, ::Error>>,

    /// Finished dependencies
    finished: BTreeMap<String, Arc<Bind>>,

    // TODO
    // feels weird to have this here, but it's in-line with making
    // matching Patterns first-class
    /// Paths being considered
    paths: Arc<Vec<PathBuf>>,
}

impl Scheduler {
    pub fn new(configuration: Arc<Configuration>) -> Scheduler {
        Scheduler {
            configuration: configuration,
            rules: HashMap::new(),
            graph: Graph::new(),
            dependencies: BTreeMap::new(),
            waiting: Vec::new(),
            pending: Vec::new(),
            finished: BTreeMap::new(),
            paths: Arc::new(Vec::new()),
        }
    }

    // TODO
    // it's probably beneficial to keep this stuff here
    // that way the files are only enumerated once and each handler
    // sees the same set of files, makes things slightly more
    // deterministic?

    /// Re-enumerate the paths in the input directory
    pub fn update_paths(&mut self) {
        use walkdir::WalkDir;
        use walkdir::WalkDirIterator;

        let walked_paths =
            WalkDir::new(&self.configuration.input)
                .into_iter()
                .filter_entry(|entry| {
                    if let Some(ref ignore) = self.configuration.ignore {
                        let file_name = &Path::new(entry.path().file_name().unwrap());

                        if ignore.matches(file_name) {
                            return false;
                        }
                    }

                    true
                })
                .filter_map(|entry| entry.ok())
                .filter_map(|entry| {
                    entry.metadata()
                        .map(|m| {
                            if m.is_file() { Some(entry.path().to_path_buf()) }
                            else { None }
                        })
                        .unwrap_or(None)
                })
                .collect();

        self.paths = Arc::new(walked_paths);
    }

    pub fn add(&mut self, rule: Arc<Rule>) {
        // prepare bind-data with the name and configuration
        let data = bind::Data::new(
            String::from(rule.name()),
            self.configuration.clone());
        let name = data.name.clone();

        // TODO
        // instead of rule_count == 0,
        // check if self.waiting.is_empty()?

        // construct job from bind-data, rule kind, rule handler, and paths
        // push it to waiting queue
        self.waiting.push(Job::new(data, rule.handler()));

        self.graph.add_node(name.clone());

        // make its dependencies depend on this binding
        for dep in rule.dependencies() {
            self.graph.add_edge(dep.clone(), name.clone());
        }

        self.rules.insert(name.clone(), rule);
    }

    // TODO: will need Borrow bound
    // TODO
    // should send the finished bind to a result channel
    // this will enable decoupling of cli status messages
    // from the core library
    fn satisfy(&mut self, current: Bind) {
        let bind_name = current.name.clone();

        // if they're done, move from staging to finished
        self.finished.insert(bind_name.clone(), Arc::new(current));

        if let Some(dependents) = self.graph.dependents_of(&bind_name) {
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
               .partition(|job| self.dependencies[&job.bind.name] == 0);

        self.waiting = waiting;

        ready
    }

    pub fn sort_jobs(&mut self, order: VecDeque<String>) {
        assert!(self.waiting.len() == order.len(),
                "`waiting` and `order` are not the same length");

        let mut job_map =
            mem::replace(&mut self.waiting, Vec::new())
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

                *self.dependencies.entry(name).or_insert(0) += count;

                return job;
            })
            .collect::<Vec<Job>>();

        mem::replace(&mut self.waiting, ordered);

        assert!(job_map.is_empty(), "not all jobs were sorted!");
    }

    pub fn build(&mut self) -> ::Result<()> {
        use util::handle::bind::InputPaths;

        if self.waiting.is_empty() {
            println!("there is nothing to do");
            return Ok(());
        }

        for job in &mut self.waiting {
            job.bind.extensions.write().unwrap()
                .insert::<InputPaths>(self.paths.clone());
        }

        let cpu_pool = CpuPool::new_num_cpus();

        // NOTE
        //
        // using futures_cpupool
        //
        // * For each ready job, spawn it on the cpupool and add it to a vector
        // of pending jobs.
        //
        // * In the main loop perform a select_all().wait() on the vector to
        // wait for the first available job. select_all() returns a triple of:
        //
        //   1. resolved value
        //   2. index of resolved future within pending list
        //   3. original vector, with resolved future removed
        //
        // * When a future is resolve (i.e. job is ready), enqueue all ready
        // other ready jobs

        let order = try!(self.graph.resolve_all());

        self.sort_jobs(order);
        self.enqueue_ready(&cpu_pool);

        while !self.pending.is_empty() {
            let pending = mem::replace(&mut self.pending, Vec::new());

            match futures::select_all(pending).wait() {
                Ok((bind, _index, new_pending)) => {
                    let mut new_pending_boxed =
                        new_pending.into_iter().map(|f| f.boxed()).collect();

                    mem::swap(&mut new_pending_boxed, &mut self.pending);

                    self.enqueue_ready(&cpu_pool);
                    self.satisfy(bind);
                }
                Err((e, _index, _new_pending)) => {
                    return Err(
                        From::from(
                            format!("a job panicked. stopping everything:\n{}", e)));
                }
            }
        }

        // TODO
        // no longer necessary post-partial update purge?
        self.reset();

        Ok(())
    }

    // TODO: audit
    fn reset(&mut self) {
        self.graph = Graph::new();
        self.waiting.clear();
    }

    fn enqueue_ready(&mut self, cpu_pool: &CpuPool) {
        for mut job in self.ready() {
            let name = job.bind.name.clone();

            if let Some(deps) = self.graph.dependencies_of(&name) {
                // insert each dependency
                for dep in deps {
                    // mutation of the bind dependencies is what necessitates
                    // Job using a bind::Data and only building the
                    // actual Bind on-the-fly, instead of only dealing with
                    // a Bind
                    job.bind.dependencies.insert(dep.clone(), self.finished[dep].clone());
                }
            }

            let boxed_future = futures::lazy(move || job.process());
            let spawned_future = cpu_pool.spawn(boxed_future).boxed();
            self.pending.push(spawned_future);
        }
    }
}
