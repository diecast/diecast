//! Site generation.

use std::sync::{Arc, Mutex, TaskPool};
use std::collections::HashMap;
use std::collections::hash_map::{Vacant, Occupied};
use std::fmt::{mod, Show};

use pattern::Pattern;
use compiler::{mod, Compile, Compiler, Chain};
use compiler::Status::{Paused, Done};
use item::{Item, Dependencies};
use dependency::Graph;

pub struct Job {
  pub id: uint,
  pub binding: &'static str,

  pub item: Item,
  pub compiler: Compiler,
  pub dependency_count: uint,
  pub dependencies: Option<Dependencies>,
}

impl Show for Job {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "#{} [{}] {}, depends on: {}, dependency_count: {}",
           self.id,
           self.binding,
           self.item,
           self.dependencies,
           self.dependency_count)
  }
}

impl Job {
  pub fn new(binding: &'static str,
             item: Item,
             compiler: Compiler,
             id: uint)
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

  /// Mapping the bind name to its items
  bindings: HashMap<&'static str, Vec<uint>>,

  /// Keeps track of what binding depends on what
  dependencies: HashMap<&'static str, Vec<&'static str>>,

  /// The jobs
  jobs: Vec<Job>,

  /// Dependency resolution
  graph: Graph,
}

impl Site {
  pub fn new(input: Path, output: Path) -> Site {
    use std::io::fs::PathExtensions;
    use std::io::fs;

    let paths =
      fs::walk_dir(&input).unwrap()
        .filter(|p| p.is_file())
        .collect::<Vec<Path>>();

    Site {
      input: input,
      output: output,

      paths: paths,

      bindings: HashMap::new(),
      dependencies: HashMap::new(),
      jobs: Vec::new(),
      graph: Graph::new(),
    }
  }
}

impl Site {
  // ALTERNATIVE
  // have graph operate on binding names
  // this cuts down on node count
  pub fn build(mut self) {
    match self.graph.resolve() {
      Ok(order) => {
        use std::mem;

        // trick the stupid borrowck
        let mut optioned =
          mem::replace(&mut self.jobs, Vec::new())
            .into_iter()
            .map(Some)
            .collect::<Vec<Option<Job>>>();

        // put the jobs into the order provided
        let ordered =
          order.iter()
            .map(|&index| {
              let mut job = optioned[index].take().unwrap();
              job.dependency_count = self.graph.dependency_count(index);
              return job;
            })
            .collect::<Vec<Job>>();

        println!("order: {}", ordered);

        let total_jobs = ordered.len();
        let task_pool = TaskPool::new(::std::os::num_cpus());
        let (job_tx, job_rx) = channel();
        let (result_tx, result_rx) = channel();
        let job_rx = Arc::new(Mutex::new(job_rx));
        let (ready, mut waiting) =
          ordered.partition(|ref job| job.dependency_count == 0);

        println!("jobs: {}", total_jobs);

        println!("ready: {}", ready);

        for job in ready.into_iter() {
          job_tx.send(job);
        }

        let mut completed = 0u;

        for i in range(0, total_jobs) {
          println!("loop {}", i);
          let result_tx = result_tx.clone();
          let job_rx = job_rx.clone();

          task_pool.execute(move || {
            let mut job = job_rx.lock().recv();
            job.process();
            result_tx.send(job);
          });
        }

        // Builds up the dependencies for an Item as they are built.
        //
        // Since multiple items may depend on the same Item, an Arc
        // is used to avoid having to clone it each time, since the
        // dependencies will be immutable anyways.
        let mut finished_deps: HashMap<&'static str, Arc<Vec<Item>>> =
          HashMap::new();
        let mut ready_deps: HashMap<&'static str, Dependencies> =
          HashMap::new();
        let mut paused: HashMap<&'static str, Vec<Job>> =
          HashMap::new();

        while completed < total_jobs {
          println!("waiting. completed: {} total: {}", completed, total_jobs);
          let current = result_rx.recv();
          println!("received");

          match current.compiler.status {
            Paused => {
              println!("paused {}", current.id);

              let total = self.bindings[current.binding].len();
              let binding = current.binding.clone();

              let finished = match paused.entry(binding) {
                Vacant(entry) => {
                  entry.set(vec![current]);
                  1 == total
                },
                Occupied(mut entry) => {
                  entry.get_mut().push(current);
                  entry.get().len() == total
                },
              };

              println!("paused so far ({}): {}",
                       paused[binding].len(),
                       paused[binding]);
              println!("total to pause: {}", total);
              println!("finished: {}", finished);

              if finished {
                let jobs = paused.remove(binding).unwrap();

                println!("checking dependencies of \"{}\"", binding);
                println!("current dependencies: {}", self.dependencies);

                let mut grouped = HashMap::new();

                for mut job in jobs.into_iter() {
                  match grouped.entry(job.binding) {
                    Vacant(entry) => {
                      entry.set(vec![job]);
                    },
                    Occupied(mut entry) => {
                      entry.get_mut().push(job);
                    },
                  }
                }

                let keys =
                  grouped.keys()
                    .map(|s| s.clone())
                    .collect::<Vec<&'static str>>();

                for &binding in keys.iter() {
                  let mut deps = HashMap::new();

                  let currents = grouped.remove(binding).unwrap();
                  let saved =
                    Arc::new(currents.iter()
                               .map(|j| j.item.clone())
                               .collect::<Vec<Item>>());

                  for mut job in currents.into_iter() {
                    println!("re-enqueuing: {}", job);

                    let cur_deps = match deps.entry(binding) {
                      Vacant(entry) => {
                        let mut hm = HashMap::new();
                        hm.insert(binding, Arc::new(vec![job.item.clone()]));

                        if let Some(old_deps) = job.dependencies {
                          for (binding, the_deps) in old_deps.iter() {
                            println!("loop: {} - {}", binding, the_deps);
                            hm.insert(*binding, the_deps.clone());
                          }
                        }

                        hm.insert(binding, saved.clone());

                        let arc_map = Arc::new(hm);
                        let cloned = arc_map.clone();
                        entry.set(arc_map);
                        cloned
                      },
                      Occupied(entry) => {
                        entry.get().clone()
                      },
                    };

                    job.dependencies = Some(cur_deps);
                    job_tx.send(job);

                    let result_tx = result_tx.clone();
                    let job_rx = job_rx.clone();

                    task_pool.execute(move || {
                      let mut job = job_rx.lock().recv();
                      job.process();
                      result_tx.send(job);
                    });
                  }
                }
              }
            },
            Done => {
              println!("finished {}", current.id);

              // decrement dependencies of jobs
              println!("before waiting: {}", waiting);

              let binding = current.binding.clone();

              if let Some(dependents) = self.graph.dependents_of(current.id) {
                for job in waiting.iter_mut() {
                  if dependents.contains(&job.id) {
                    job.dependency_count -= 1;
                  }
                }
              }

              match finished_deps.entry(binding) {
                Vacant(entry) => {
                  entry.set(Arc::new(vec![current.item]));
                },
                Occupied(mut entry) => {
                  entry.get_mut().make_unique().push(current.item);
                },
              }

              println!("after waiting: {}", waiting);

              // split the waiting vec again
              let (ready, waiting_) =
                waiting.partition(|ref job| job.dependency_count == 0);
              waiting = waiting_;

              println!("now ready: {}", ready);

              for mut job in ready.into_iter() {
                let deps = match ready_deps.entry(binding) {
                  Vacant(entry) => {
                    let mut deps = HashMap::new();

                    println!("checking dependencies of \"{}\"", binding);
                    println!("current dependencies: {}", self.dependencies);

                    for &dep in self.dependencies[job.binding].iter() {
                      println!("getting finished \"{}\"", dep);
                      deps.insert(dep, finished_deps[dep].clone());
                    }

                    let arc_deps = Arc::new(deps);
                    let cloned = arc_deps.clone();

                    entry.set(arc_deps);
                    cloned
                  },
                  Occupied(entry) => {
                    entry.get().clone()
                  },
                };

                job.dependencies = Some(deps);
                job_tx.send(job);
              }

              completed += 1;
              println!("completed {}", completed);
            },
          }
        }
      },
      Err(cycle) => {
        panic!("a dependency cycle was detected: {}", cycle);
      },
    }
  }

  // get the paths: Vec<Path> once
  // whenever something is bound, push a new Job for every match

  // I think the graph should construct the Jobs since it knows both the index and dependency count
  // I can't just set the dependency count to dependencies.len() in case the user specified
  // redundant or incorrect dependencies
  // to do this the graph can 

  fn add_job(&mut self,
             binding: &'static str,
             item: Item,
             compiler: Compiler,
             dependencies: &Option<Vec<&'static str>>) {
    let index = self.jobs.len();
    self.jobs.push(Job::new(binding, item, compiler, index));

    match self.bindings.entry(binding) {
      Vacant(entry) => { entry.set(vec![index]); },
      Occupied(mut entry) => { entry.get_mut().push(index); },
    }

    if let &Some(ref deps) = dependencies {
      for &dep in deps.iter() {
        for &id in self.bindings[dep].iter() {
          // index depends on id, so id must be done before index
          // so the edge goes: id -> index
          self.graph.add_edge(id, index);
        }
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

      if let Some(deps) = binding.dependencies {
        self.dependencies.insert(binding.name, deps);
      }

      return self;
  }

  pub fn matching<P>(mut self, pattern: P, binding: Rule) -> Site
    where P: Pattern {
      use std::mem;

      // stupid hack to trick borrowck
      let paths = mem::replace(&mut self.paths, Vec::new());

      for path in paths.iter() {
        let relative = &path.path_relative_from(&self.input).unwrap();

        if pattern.matches(relative) {
          self.add_job(
            binding.name,
            Item::new(Some(path.clone()), None),
            binding.compiler.clone(),
            &binding.dependencies);
        }
      }

      mem::replace(&mut self.paths, paths);

      if let Some(deps) = binding.dependencies {
        self.dependencies.insert(binding.name, deps);
      }

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

