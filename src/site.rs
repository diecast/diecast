//! Site generation.

use std::sync::{Arc, Mutex, TaskPool};
use std::sync::mpsc::channel;
use std::collections::HashMap;
use std::collections::hash_map::Entry::{Vacant, Occupied};
use std::fmt;

use pattern::Pattern;
use compiler::{self, Compile, Compiler, Chain};
use compiler::Status::{Paused, Done};
use item::{Item, Dependencies};
use dependency::Graph;

pub struct Job {
    pub id: usize,
    pub binding: usize,

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
    pub fn new(binding: usize,
               item: Item,
               compiler: Compiler,
               id: usize)
        -> Job {
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

/// A Site scans the input path to find
/// files that match the given pattern. It then
/// takes each of those files and passes it through
/// the compiler chain.
pub struct Site {
    /// The input directory
    input: Path,

    /// The output directory
    output: Path,

    /// The collected paths in the input directory
    paths: Vec<Path>,

    /// Mapping the id to its dependencies
    bindings: HashMap<usize, Vec<usize>>,

    /// Mapping the name to its id
    ids: HashMap<&'static str, usize>,

    /// The jobs
    jobs: Vec<Job>,

    /// Dependency resolution
    graph: Graph,
}

impl Site {
    pub fn new(input: Path, output: Path) -> Site {
        use std::old_io::fs::PathExtensions;
        use std::old_io::fs;

        let paths =
            fs::walk_dir(&input).unwrap()
            .filter(|p| p.is_file())
            .collect::<Vec<Path>>();

        Site {
            input: input,
            output: output,

            paths: paths,

            bindings: HashMap::new(),
            ids: HashMap::new(),
            jobs: Vec::new(),
            graph: Graph::new(),
        }
    }
}

impl Site {
    pub fn build(mut self) {
        match self.graph.resolve() {
            Ok(order) => {
                use std::mem;

                // trick the stupid borrowck
                let mut unordered = mem::replace(&mut self.jobs, Vec::new());
                let mut ordered = vec![];

                for bind_id in order {
                    let (matched, rest): (Vec<Job>, Vec<Job>)
                        = unordered.into_iter().partition(|ref job| job.binding == bind_id);

                    for mut job in matched {
                        job.dependency_count = self.graph.dependency_count(job.binding);
                        ordered.push(job);
                    }

                    unordered = rest;
                }

                trace!("ordered: {:?}", ordered);

                let total_jobs = ordered.len();
                let task_pool = TaskPool::new(::std::os::num_cpus());
                let (job_tx, job_rx) = channel();
                let (result_tx, result_rx) = channel();
                let job_rx = Arc::new(Mutex::new(job_rx));
                let (ready, mut waiting): (Vec<Job>, Vec<Job>) =
                   ordered.into_iter().partition(|ref job| job.dependency_count == 0);

                trace!("jobs: {}", total_jobs);

                trace!("ready: {:?}", ready);

                for job in ready {
                    job_tx.send(job).unwrap();
                }

                let mut completed = 0us;

                for i in range(0, total_jobs) {
                    trace!("loop {}", i);
                    let result_tx = result_tx.clone();
                    let job_rx = job_rx.clone();

                    task_pool.execute(move || {
                        let mut job = job_rx.lock().unwrap().recv().unwrap();
                        job.process();
                        result_tx.send(job).unwrap();
                    });
                }

                // Builds up the dependencies for an Item as they are built.
                //
                // Since multiple items may depend on the same Item, an Arc
                // is used to avoid having to clone it each time, since the
                // dependencies will be immutable anyways.
                let mut staging_deps: HashMap<usize, Vec<Item>> =
                    HashMap::new();
                let mut finished_deps: HashMap<usize, Arc<Vec<Item>>> =
                    HashMap::new();
                // TODO: is ready_deps now obsolete because finished_deps is
                //       ONLY for finished dependencies?
                let mut ready_deps: HashMap<usize, Dependencies> =
                    HashMap::new();
                let mut paused: HashMap<usize, Vec<Job>> =
                    HashMap::new();

                while completed < total_jobs {
                    trace!("waiting. completed: {} total: {}", completed, total_jobs);
                    let current = result_rx.recv().unwrap();
                    trace!("received");

                    match current.compiler.status {
                        Paused => {
                            trace!("paused {}", current.id);

                            let total = self.bindings[current.binding].len();
                            let binding = current.binding;

                            let finished = match paused.entry(binding) {
                                Vacant(entry) => {
                                    entry.insert(vec![current]);
                                    1 == total
                                },
                                Occupied(mut entry) => {
                                    entry.get_mut().push(current);
                                    entry.get().len() == total
                                },
                            };

                            trace!("paused so far ({}): {:?}",
                                     paused[binding].len(),
                                     paused[binding]);
                            trace!("total to pause: {}", total);
                            trace!("finished: {}", finished);

                            if finished {
                                let jobs = paused.remove(&binding).unwrap();

                                trace!("checking dependencies of \"{}\"", binding);

                                let mut grouped = HashMap::new();

                                for job in jobs {
                                    match grouped.entry(job.binding) {
                                        Vacant(entry) => {
                                            entry.insert(vec![job]);
                                        },
                                        Occupied(mut entry) => {
                                            entry.get_mut().push(job);
                                        },
                                    }
                                }

                                let keys =
                                    grouped.keys()
                                    .map(|s| s.clone())
                                    .collect::<Vec<usize>>();

                                for &binding in &keys {
                                    let mut deps = HashMap::new();

                                    let currents = grouped.remove(&binding).unwrap();
                                    let saved =
                                        Arc::new(currents.iter()
                                                 .map(|j| j.item.clone())
                                                 .collect::<Vec<Item>>());

                                    for mut job in currents {
                                        trace!("re-enqueuing: {:?}", job);

                                        let cur_deps = match deps.entry(binding) {
                                            Vacant(entry) => {
                                                let mut hm = HashMap::new();
                                                hm.insert(binding, Arc::new(vec![job.item.clone()]));

                                                if let Some(old_deps) = job.dependencies {
                                                    for (binding, the_deps) in old_deps.iter() {
                                                        trace!("loop: {} - {:?}", binding, the_deps);
                                                        hm.insert(*binding, the_deps.clone());
                                                    }
                                                }

                                                hm.insert(binding, saved.clone());

                                                let arc_map = Arc::new(hm);
                                                let cloned = arc_map.clone();
                                                entry.insert(arc_map);
                                                cloned
                                            },
                                            Occupied(entry) => {
                                                entry.get().clone()
                                            },
                                        };

                                        job.dependencies = Some(cur_deps);
                                        job_tx.send(job).unwrap();

                                        let result_tx = result_tx.clone();
                                        let job_rx = job_rx.clone();

                                        task_pool.execute(move || {
                                            let mut job = job_rx.lock().unwrap().recv().unwrap();
                                            job.process();
                                            result_tx.send(job).unwrap();
                                        });
                                    }
                                }
                            }
                        },
                        Done => {
                            trace!("finished {}", current.id);

                            trace!("before waiting: {:?}", waiting);

                            let binding = current.binding.clone();

                            // add to collection of finished items
                            let done_so_far = match staging_deps.entry(binding) {
                                Vacant(entry) => {
                                    entry.insert(vec![current.item]);
                                    1
                                },
                                Occupied(mut entry) => {
                                    entry.get_mut().push(current.item);
                                    entry.get().len()
                                },
                            };

                            let total = self.bindings[binding].len();

                            trace!("checking if done");
                            // if they're done, move from staging to finished
                            if done_so_far == total {
                                trace!("binding {} finished", binding);
                                let deps = staging_deps.remove(&binding).unwrap();
                                finished_deps.insert(binding, Arc::new(deps));

                                if let Some(dependents) = self.graph.dependents_of(current.binding) {
                                    // TODO: change to &mut waiting
                                    for job in &mut waiting {
                                        if dependents.contains(&job.binding) {
                                            job.dependency_count -= 1;
                                        }
                                    }

                                    trace!("finished_deps: {:?}", finished_deps);

                                    // decrement dependencies of jobs
                                    // split the waiting vec again
                                    let (ready, waiting_): (Vec<Job>, Vec<Job>) =
                                        waiting.into_iter().partition(|ref job| job.dependency_count == 0);
                                    waiting = waiting_;

                                    for mut job in ready {
                                        let deps = match ready_deps.entry(binding) {
                                            Vacant(entry) => {
                                                let mut deps = HashMap::new();

                                                for &dep in self.graph.dependencies_of(job.binding).unwrap() {
                                                    trace!("adding dependency: {:?}", dep);
                                                    deps.insert(dep, finished_deps[dep].clone());
                                                }

                                                let arc_deps = Arc::new(deps);
                                                let cloned = arc_deps.clone();

                                                entry.insert(arc_deps);
                                                cloned
                                            },
                                            Occupied(entry) => {
                                                entry.get().clone()
                                            },
                                        };

                                        job.dependencies = Some(deps);
                                        job_tx.send(job).unwrap();
                                    }
                                }
                            }

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

    // TODO: ensure can only add dependency on &Dependency to avoid
    // string errors
    fn add_job(&mut self,
               binding: &'static str,
               item: Item,
               compiler: Compiler,
               dependencies: &Option<Vec<&'static str>>) {
        let bind_count = self.ids.len();
        let bind_index: usize =
            *self.ids.entry(binding).get().unwrap_or_else(|v| v.insert(bind_count));

        trace!("adding job for {:?}", item);
        trace!("bind index: {}", bind_index);

        let index = self.jobs.len();
        trace!("index: {}", index);
        self.jobs.push(Job::new(bind_index, item, compiler, index));

        self.bindings.entry(bind_index).get().unwrap_or_else(|v| v.insert(vec![])).push(index);

        trace!("bindings: {:?}", self.bindings);

        self.graph.add_node(bind_index);

        if let &Some(ref deps) = dependencies {
            trace!("has dependencies: {:?}", deps);
            for &dep in deps {
                let bind_idx = self.ids[dep];
                trace!("setting dependency {} -> {}", bind_idx, bind_index);
                self.graph.add_edge(bind_idx, bind_index);
            }
        }
    }

    pub fn creating(mut self, path: Path, binding: Rule) -> Site {
        let compiler = binding.compiler;
        let target = self.output.join(path);

        self.add_job(
            binding.name,
            Item::new(None, Some(target)),
            compiler,
            &binding.dependencies);

        return self;
    }

    pub fn matching<P>(mut self, pattern: P, binding: Rule) -> Site
    where P: Pattern {
        use std::mem;

        // stupid hack to trick borrowck
        let paths = mem::replace(&mut self.paths, Vec::new());

        for path in &paths {
            let relative = &path.path_relative_from(&self.input).unwrap();

            if pattern.matches(relative) {
                self.add_job(
                    binding.name,
                    // TODO: make Vec<Path> -> Vec<Arc<Path>> to avoid copying?
                    Item::new(Some(path.clone()), None),
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

