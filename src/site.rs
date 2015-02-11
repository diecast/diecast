//! Site generation.

use std::sync::{Arc, TaskPool};
use std::sync::mpsc::{Sender, Receiver, channel};
use std::collections::{BTreeMap, RingBuf};
use std::collections::btree_map::Entry::{Vacant, Occupied};
use std::old_io::{fs, TempDir};
// use std::old_io::net::ip::SocketAddr;
use std::fmt;

use pattern::Pattern;
use compiler::{self, Compile, Compiler, Chain};
use compiler::Status::{Paused, Done};
use item::{Item, Dependencies};
use dependency::Graph;

pub struct Job {
    pub id: JobId,
    pub binding: BindingId,

    pub item: Item,
    pub compiler: Compiler,
    pub dependency_count: usize,
    pub dependencies: Option<Dependencies>,
}

impl fmt::Debug for Job {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "#{} [{}] {:?}, depends on: {:?}, dependency_count: {}",
               self.id,
               self.binding,
               self.item,
               self.dependencies,
               self.dependency_count)
    }
}

impl Job {
    pub fn new(binding: BindingId, item: Item, compiler: Compiler, id: JobId) -> Job {
        Job {
            id: id,
            binding: binding,
            item: item,
            compiler: compiler,
            dependency_count: 0,
            dependencies: None,
        }
    }

    fn process(&mut self) {
        self.compiler.compile(&mut self.item, self.dependencies.clone());
    }
}

/// The configuration of the build
/// an Arc of this is given to each Item
pub struct Configuration {
    /// The input directory
    pub input: Path,

    /// The output directory
    output: Path,

    /// The number of cpu count
    pub threads: usize,

    // TODO: necessary?
    // The cache directory
    // cache: Path,

    /// a global pattern used to ignore files and paths
    ///
    /// the following are from hakyll
    /// e.g.
    /// config.ignore = not!(regex!("^\.|^#|~$|\.swp$"))
    ignore: Option<Box<Pattern + Sync + Send>>,

    /// Whether we're in preview mode
    preview_dir: Option<TempDir>,

    // Socket on which to listen when in preview mode
    // socket_addr: SocketAddr
}

impl Configuration {
    pub fn new(input: Path, output: Path) -> Configuration {
        Configuration {
            input: input,
            output: output,
            threads: ::std::os::num_cpus(),
            ignore: None,
            preview_dir: None,
        }
    }

    pub fn thread_count(mut self, count: usize) -> Configuration {
        self.threads = count;
        self
    }

    pub fn ignore<P>(mut self, pattern: P) -> Configuration
    where P: Pattern + Sync + Send {
        self.ignore = Some(Box::new(pattern));
        self
    }

    pub fn preview(mut self, is_preview: bool) -> Configuration {
        if self.preview_dir.is_some() == is_preview {
            return self;
        }

        if is_preview {
            self.preview_dir = Some(TempDir::new(self.output.filename_str().unwrap()).unwrap());
        } else {
            self.preview_dir = None;
        }

        self
    }

    pub fn output(&self) -> &Path {
        if let Some(ref temp) = self.preview_dir {
            temp.path()
        } else {
            &self.output
        }
    }
}

/// A Site scans the input path to find
/// files that match the given pattern. It then
/// takes each of those files and passes it through
/// the compiler chain.
pub struct Site {
    configuration: Arc<Configuration>,

    /// The collected paths in the input directory
    paths: Vec<Path>,

    /// Mapping the id to its dependencies
    bindings: BTreeMap<BindingId, Vec<JobId>>,

    /// Mapping the name to its id
    ids: BTreeMap<&'static str, BindingId>,

    /// Mapping the id to its name
    names: BTreeMap<BindingId, &'static str>,

    /// The jobs
    jobs: Vec<Job>,

    /// Dependency resolution
    graph: Graph,

    /// Thread pool to process jobs
    thread_pool: TaskPool,

    /// For worker threads to send result back to main
    result_tx: Sender<Job>,

    /// For main thread to receive results
    result_rx: Receiver<Job>,

    /// the dependencies as they're being built
    staging_deps: BTreeMap<BindingId, Vec<Item>>,

    /// finished dependencies
    finished_deps: BTreeMap<BindingId, Arc<Vec<Item>>>,

    /// dependencies that are currently paused due to a barrier
    paused: BTreeMap<BindingId, Vec<Job>>,

    /// items whose dependencies have not been fulfilled
    waiting: Vec<Job>,
}

impl Site {
    pub fn new(configuration: Configuration) -> Site {
        use std::old_io::fs::PathExtensions;
        use std::old_io::fs;

        let paths =
            fs::walk_dir(&configuration.input).unwrap()
            .filter(|p| {
                let is_ignored = if let Some(ref pattern) = configuration.ignore {
                    pattern.matches(&Path::new(p.filename_str().unwrap()))
                } else {
                    false
                };

                p.is_file() && !is_ignored
            })
            .collect::<Vec<Path>>();

        let threads = configuration.threads;

        trace!("output directory is: {:?}", configuration.output());

        trace!("using {} threads", threads);

        // channels for sending and receiving results
        let (result_tx, result_rx) = channel();

        Site {
            configuration: Arc::new(configuration),

            paths: paths,

            bindings: BTreeMap::new(),
            ids: BTreeMap::new(),
            names: BTreeMap::new(),
            jobs: Vec::new(),
            graph: Graph::new(),

            thread_pool: TaskPool::new(threads),
            result_tx: result_tx,
            result_rx: result_rx,

            staging_deps: BTreeMap::new(),
            finished_deps: BTreeMap::new(),
            paused: BTreeMap::new(),

            waiting: Vec::new(),
        }
    }
}

fn sort_jobs(jobs: &mut Vec<Job>, order: &RingBuf<BindingId>, graph: &Graph) {
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

pub type BindingId = usize;
pub type JobId = usize;

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
        let mut new_deps =
            if let Some(ref old_deps) = jobs[0].dependencies {
                (**old_deps).clone()
            } else {
                BTreeMap::new()
            };

        // insert the frozen state of these jobs
        new_deps.insert(self.names[binding], {
            Arc::new(jobs.iter()
                .map(|j| j.item.clone())
                .collect::<Vec<Item>>())
        });

        trace!("checking dependencies of \"{}\"", binding);

        let arc_map = Arc::new(new_deps);

        // package each job with its dependencies
        for mut job in jobs {
            job.dependencies = Some(arc_map.clone());

            trace!("re-enqueuing: {:?}", job);

            self.dispatch_job(job);
        }
    }

    fn handle_done(&mut self, current: Job) {
        trace!("finished {}", current.id);
        trace!("before waiting: {:?}", self.waiting);

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
            return;
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
            return;
        }

        // prepare dependencies for all ready dependents

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
        let mut dependents = ready.iter().map(|j| j.binding).collect::<Vec<usize>>();

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
                deps.insert(self.names[dep], self.finished_deps[dep].clone());
            }

            deps_cache.insert(self.names[dependent], Arc::new(deps));
        }

        // attach the dependencies and dispatch
        for mut job in ready {
            job.dependencies = Some(deps_cache[self.names[job.binding]].clone());
            trace!("job now ready: {:?}", job);
            self.dispatch_job(job)
        }
    }

    pub fn build(&mut self) {
        match self.graph.resolve() {
            Ok(order) => {
                use std::mem;

                // create the output directory
                fs::mkdir_recursive(self.configuration.output(), ::std::old_io::USER_RWX)
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

                // jobs completed so far
                let mut completed = 0us;

                while completed < total_jobs {
                    let current = self.result_rx.recv().unwrap();

                    trace!("waiting. completed: {} total: {}", completed, total_jobs);
                    trace!("received");

                    match current.compiler.status {
                        Paused => {
                            self.handle_paused(current);
                        },
                        Done => {
                            self.handle_done(current);
                            completed += 1;
                            trace!("completed {}", completed);
                        },
                    }
                }
            },
            Err(cycle) => {
                panic!("a dependency cycle was detected: {:?}", cycle);
            },
        }

        trace!("THIS IS A DEBUG MESSAGE");
    }

    // TODO: ensure can only add dependency on &Dependency to avoid string errors
    fn add_job(&mut self,
               binding: &'static str,
               item: Item,
               compiler: Compiler,
               dependencies: &Option<Vec<&'static str>>) {
        // create or get an id for this name
        let bind_count = self.ids.len();
        let bind_index: BindingId =
            *self.ids.entry(binding).get()
                .unwrap_or_else(|v| v.insert(bind_count));

        self.names.insert(bind_index, binding);

        trace!("adding job for {:?}", item);
        trace!("bind index: {}", bind_index);

        // create a job id
        let index = self.jobs.len();
        trace!("index: {}", index);

        // add the job to the list of jobs
        self.jobs.push(Job::new(bind_index, item, compiler, index));

        // associate the job id with the binding
        self.bindings.entry(bind_index).get()
            .unwrap_or_else(|v| v.insert(vec![]))
            .push(index);

        trace!("bindings: {:?}", self.bindings);

        // add the binding to the graph
        self.graph.add_node(bind_index);

        // if there are dependencies, create an edge from the dependency to this binding
        if let &Some(ref deps) = dependencies {
            trace!("has dependencies: {:?}", deps);

            for &dep in deps {
                let dependency_id = self.ids[dep];
                trace!("setting dependency {} -> {}", dependency_id, bind_index);
                self.graph.add_edge(dependency_id, bind_index);
            }
        }
    }

    pub fn creating(mut self, path: Path, binding: Rule) -> Site {
        let compiler = binding.compiler;
        let target = self.configuration.output().join(path);

        let conf = self.configuration.clone();

        self.add_job(
            binding.name,
            Item::new(conf, None, Some(target)),
            compiler,
            &binding.dependencies);

        return self;
    }

    pub fn matching<P>(mut self, pattern: P, binding: Rule) -> Site
    where P: Pattern {
        use std::mem;

        let paths = mem::replace(&mut self.paths, Vec::new());

        for path in &paths {
            let relative = path.path_relative_from(&self.configuration.input).unwrap();

            // TODO:
            // the Item needs the actual path, so it can go ahead and read
            // but it also needs to be able to have the relative path,
            // so that routing works as intended

            let conf = self.configuration.clone();

            if pattern.matches(&relative) {
                self.add_job(
                    binding.name,
                    // TODO: make Vec<Path> -> Vec<Arc<Path>> to avoid copying?
                    //       what does this mean?
                    Item::new(conf, Some(relative), None),
                    binding.compiler.clone(),
                    &binding.dependencies);
            }
        }

        mem::replace(&mut self.paths, paths);

        return self;
    }
}

pub struct Rule {
    pub name: &'static str,
    pub compiler: Compiler,
    pub dependencies: Option<Vec<&'static str>>,
}

impl Rule {
    pub fn new(name: &'static str) -> Rule {
        Rule {
            name: name,
            compiler: Compiler::new(Chain::only(compiler::stub).build()),
            dependencies: None,
        }
    }

    pub fn compiler(mut self, compiler: Compiler) -> Rule {
        self.compiler = compiler;
        return self;
    }

    pub fn depends_on<D>(mut self, dependency: D) -> Rule where D: Dependency {
        let mut pushed = self.dependencies.unwrap_or_else(|| Vec::new());
        pushed.push(dependency.name());
        self.dependencies = Some(pushed);

        return self;
    }
}

pub trait Dependency {
    fn name(&self) -> &'static str;
}

impl Dependency for &'static str {
    fn name(&self) -> &'static str {
        self.clone()
    }
}

impl<'a> Dependency for &'a Rule {
    fn name(&self) -> &'static str {
        self.name.clone()
    }
}

