//! Site generation.

use sync::{Mutex, Arc};
use std::collections::{RingBuf, HashMap, Bitv};
use std::collections::hash_map::{Vacant, Occupied};
use std::sync::deque::{BufferPool, Stolen};
use std::sync::TaskPool;
use std::fmt::{mod, Show};

use pattern::Pattern;
use route::{mod, Route};
use compile::{mod, Compile};
use item::Item;
use item::Relation::{Reading, Writing, Mapping};
use dependency::Graph;

pub struct Job {
  id: u32,
  binding: &'static str,
  item: Item,
  compiler: Arc<Box<Compile + Send + Sync>>,
  dependencies: u32,
}

impl Show for Job {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{}", self.id)
  }
}

impl Job {
  pub fn new(binding: &'static str,
             item: Item,
             compiler: Arc<Box<Compile + Send + Sync>>,
             id: u32)
         -> Job {
    Job {
      id: id,
      binding: binding,
      item: item,
      compiler: compiler,
      dependencies: 0,
    }
  }

  pub fn set_id(&mut self, id: u32) {
    self.id = id;
  }

  pub fn set_dependencies(&mut self, dependencies: u32) {
    self.dependencies = dependencies;
  }

  pub fn decrement_dependencies(&mut self) {
    self.dependencies -= 1;
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

  /// Mapping the bind name to its items
  bindings: HashMap<&'static str, Vec<u32>>,

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

      bindings: HashMap::new(),
      jobs: Vec::new(),
      graph: Graph::new(),
    }
  }
}

impl Generator {
  pub fn generate(mut self) {
    // ALTERNATIVE
    // push a map of name -> binding
    // add_node for every name
    // dependency reg. operates on name, e.g. "posts"
    // once the name topological order is computed,
    //   loop through each (name, binding) in the map
    //   and do output.extend(binding.items)

    // job should include an index as given by the graph order
    // the graph should operate entirely based on the indices

    // TODO: dependency enforcement
    //       partition ordered based on dependencies = 0

    match self.graph.resolve() {
      Ok(order) => {
        let mut optioned =
          self.jobs.into_iter()
            .map(Some)
            .collect::<Vec<Option<Job>>>();

        // put the jobs into the order provided
        let ordered =
          order.iter()
            .map(|&index| optioned[index as uint].take().unwrap())
            .collect::<Vec<Job>>();

        println!("order: {}", ordered);

        let jobs = ordered.len();
        let mut job_queue = BufferPool::new();
        let (mut worker, mut stealer) = job_queue.deque();

        for job in ordered.into_iter() {
          worker.push(job);
        }

        let task_pool = TaskPool::new(4u);

        let (tx, rx) = channel();

        enum Status {
          Finished(u32),
          Error,
          Aborted,
        }

        for _ in range(0, jobs) {
          let tx = tx.clone();
          let stealer = stealer.clone();

          task_pool.execute(proc() {
            match stealer.steal() {
              Stolen::Data(job) => {
                // process job
                // tx.send(result);

                tx.send(Status::Finished(job.id));
              },
              Stolen::Empty => {
                tx.send(Status::Error);
              },
              Stolen::Abort => {
                tx.send(Status::Aborted);
              },
            }
          });
        }

        let mut completed = 0u;

        while completed < jobs {
          match rx.recv() {
            Status::Finished(id) => println!("finished {}", id),
            Status::Error => println!("errored"),
            Status::Aborted => println!("aborted"),
          }

          // if paused, worker.push(job_again)
          completed += 1;
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
             compiler: Arc<Box<Compile + Send + Sync>>,
             dependencies: &Option<Vec<&'static str>>) {
    let index = self.jobs.len() as u32;
    self.jobs.push(Job::new(binding, item, compiler, index));

    match self.bindings.entry(binding) {
      Vacant(entry) => { entry.set(vec![index]); },
      Occupied(mut entry) => { entry.get_mut().push(index); },
    }

    self.graph.add_node(index);

    if let &Some(ref deps) = dependencies {
      for dep in deps.iter() {
        for id in self.bindings[*dep].iter() {
          // id depends on index, so id must be done before index
          // so the edge goes: id -> index
          self.graph.add_edge(*id, index);
        }
      }
    }
  }

  pub fn creating(mut self, path: Path, binding: Binding) -> Generator {
      let compiler = Arc::new(binding.compiler);
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

      let compiler = Arc::new(binding.compiler);

      // stupid hack to trick borrowck
      let paths = mem::replace(&mut self.paths, Vec::new());

      for path in paths.iter() {
        let relative = &path.path_relative_from(&self.input).unwrap();

        if pattern.matches(relative) {
          self.add_job(
            binding.name,
            Item::new(Reading(path.clone())),
            compiler.clone(),
            &binding.dependencies);
        }
      }

      mem::replace(&mut self.paths, paths);

      self
  }
}

pub struct Binding {
  pub name: &'static str,
  pub compiler: Box<Compile + Send + Sync>,
  pub router: Box<Route + Send + Sync>,
  pub dependencies: Option<Vec<&'static str>>,
}

impl Binding {
  pub fn new(name: &'static str) -> Binding {
    Binding {
      name: name,
      compiler: box compile::Stub as Box<Compile + Send + Sync>,
      router: box route::Identity as Box<Route + Send + Sync>,
      dependencies: None,
    }
  }

  pub fn compiler<C>(mut self, compiler: C) -> Binding where C: Compile + Send + Sync {
    self.compiler = box compiler as Box<Compile + Send + Sync>;
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

