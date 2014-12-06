//! Site generation.

use std::sync::{Arc, Mutex, TaskPool};
use std::collections::HashMap;
use std::collections::hash_map::{Vacant, Occupied};
use std::fmt::{mod, Show};

use pattern::Pattern;
use route::{mod, Route};
use compile::{mod, Compile, Compiler, Chain};
use item::Item;
use item::Relation::{Reading, Writing};
use dependency::Graph;

use compile::Status::{Paused, Done, Stopped};

pub struct Job {
  pub id: uint,
  pub binding: &'static str,

  pub item: Item,
  pub compiler: Compiler,
  pub dependencies: uint,
}

impl Show for Job {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "#{} [{}] {}, dependencies: {}",
           self.id,
           self.binding,
           self.item,
           self.dependencies)
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
      dependencies: 0,
    }
  }

  fn process(&mut self) {
    self.compiler.compile(&mut self.item);
  }
}

/// A generator scans the input path to find
/// files that match the given pattern. It then
/// takes each of those files and passes it through
/// the compiler chain.
pub struct Generator {
  /// The input directory
  input: Path,

  /// The output directory
  output: Path,

  /// The collected paths in the input directory
  paths: Vec<Path>,

  paused: HashMap<&'static str, Vec<Job>>,

  /// Mapping the bind name to its items
  bindings: HashMap<&'static str, Vec<uint>>,

  /// The jobs
  jobs: Vec<Job>,

  /// Dependency resolution
  graph: Graph,
}

impl Generator {
  pub fn new(input: Path, output: Path) -> Generator {
    use std::io::fs::PathExtensions;
    use std::io::fs;

    let paths =
      fs::walk_dir(&input).unwrap()
        .filter(|p| p.is_file())
        .collect::<Vec<Path>>();

    Generator {
      input: input,
      output: output,

      paths: paths,

      paused: HashMap::new(),
      bindings: HashMap::new(),
      jobs: Vec::new(),
      graph: Graph::new(),
    }
  }
}

impl Generator {
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
              job.dependencies = self.graph.dependency_count(index);
              return job;
            })
            .collect::<Vec<Job>>();

        println!("order: {}", ordered);

        let total_jobs = ordered.len();
        let task_pool = TaskPool::new(4u);
        let (job_tx, job_rx) = channel();
        let (result_tx, result_rx) = channel();
        let job_rx = Arc::new(Mutex::new(job_rx));
        let (ready, mut waiting) = ordered.partition(|ref job| job.dependencies == 0);

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

          task_pool.execute(proc() {
            let mut job = job_rx.lock().recv();
            job.process();
            result_tx.send(job);
          });
        }

        // while let Ok(current) = rx.recv_opt() {
        //   println!("received");
        while completed < total_jobs {
          println!("waiting. completed: {} total: {}", completed, total_jobs);
          let current = result_rx.recv();
          println!("received");

          match current.compiler.status {
            Stopped => {
              println!("stopped {}", current.id);
            },
            Paused => {
              println!("paused {}", current.id);

              let total = self.bindings[current.binding].len();
              let binding = current.binding.clone();

              let finished = match self.paused.entry(binding) {
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
                       self.paused[binding].len(),
                       self.paused[binding]);
              println!("total to pause: {}", total);
              println!("finished: {}", finished);

              if finished {
                let jobs = self.paused.remove(binding).unwrap();

                for job in jobs.into_iter() {
                  println!("re-enqueuing: {}", job);

                  job_tx.send(job);

                  let result_tx = result_tx.clone();
                  let job_rx = job_rx.clone();

                  task_pool.execute(proc() {
                    let mut job = job_rx.lock().recv();
                    job.process();
                    result_tx.send(job);
                  });
                }
              }
            },
            Done => {
              println!("finished {}", current.id);

              // decrement dependencies of jobs
              println!("before waiting: {}", waiting);

              if let Some(dependents) = self.graph.dependents_of(current.id) {
                for job in waiting.iter_mut() {
                  if dependents.contains(&job.id) {
                    job.dependencies -= 1;
                  }
                }
              }

              println!("after waiting: {}", waiting);

              // split the waiting vec again
              let (ready, waiting_) = waiting.partition(|ref job| job.dependencies == 0);
              waiting = waiting_;

              println!("now ready: {}", ready);

              for job in ready.into_iter() {
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
          // id depends on index, so id must be done before index
          // so the edge goes: id -> index
          self.graph.add_edge(id, index);
        }
      }
    }
  }

  pub fn creating(mut self, path: Path, binding: Binding) -> Generator {
      let compiler = binding.compiler;
      let target = self.output.join(path);

      self.add_job(
        binding.name,
        Item::new(Writing(target)),
        compiler,
        &binding.dependencies);

      self
  }

  pub fn matching<P>(mut self, pattern: P, binding: Binding) -> Generator
    where P: Pattern + Send + Sync {
      use std::mem;

      // stupid hack to trick borrowck
      let paths = mem::replace(&mut self.paths, Vec::new());

      for path in paths.iter() {
        let relative = &path.path_relative_from(&self.input).unwrap();

        if pattern.matches(relative) {
          self.add_job(
            binding.name,
            Item::new(Reading(path.clone())),
            binding.compiler.clone(),
            &binding.dependencies);
        }
      }

      mem::replace(&mut self.paths, paths);

      self
  }
}

pub struct Binding {
  pub name: &'static str,
  pub compiler: Compiler,
  pub router: Box<Route + Send + Sync>,
  pub dependencies: Option<Vec<&'static str>>,
}

impl Binding {
  pub fn new(name: &'static str) -> Binding {
    Binding {
      name: name,
      compiler: Compiler::new(Chain::only(compile::stub)),
      router: box route::identity as Box<Route + Send + Sync>,
      dependencies: None,
    }
  }

  pub fn compiler(mut self, chain: Chain) -> Binding {
    self.compiler = Compiler::new(chain);
    return self;
  }

  pub fn router<R>(mut self, router: R) -> Binding where R: Route + Send + Sync {
    self.router = box router as Box<Route + Send + Sync>;
    return self;
  }

  pub fn dependencies(mut self, dependencies: Vec<&'static str>) -> Binding {
    self.dependencies = Some(dependencies);
    return self;
  }
}

