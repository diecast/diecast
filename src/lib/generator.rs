//! Site generation.

use sync::{Mutex, Arc};
use std::collections::{RingBuf, HashMap};
use std::collections::hash_map::{Vacant, Occupied};
use std::fmt::{mod, Show};

use pattern::Pattern;
use route::{mod, Route};
use compile::{mod, Compile};
use item::Item;
use item::Relation::{Reading, Writing, Mapping};
use dependency::Graph;

pub struct Job {
  id: i32,
  binding: &'static str,
  item: Item,
  compiler: Arc<Box<Compile + Send + Sync>>,
  dependencies: i32,
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
             id: i32)
         -> Job {
    Job {
      id: id,
      binding: binding,
      item: item,
      compiler: compiler,
      dependencies: 0,
    }
  }

  pub fn set_id(&mut self, id: i32) {
    self.id = id;
  }

  pub fn set_dependencies(&mut self, dependencies: i32) {
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
  bindings: HashMap<&'static str, Vec<i32>>,

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
    // add a node for every graph
    // the problem is that we want to store only the indexes?
    // that way we can simply take them and put the existing vector
    // in that order by iterating through half of the indices and using
    // .as_mut_slice().swap(i, i * 2)
    //
    // in order to use indices, we need to maintain a map of Item to index?
    //
    // alternatively we could store Rc<T>, then when the graph returns
    // the result, we could use std::rc::try_unwrap to unwrap them
    // this seems much more straightforward? would have to use Weak<T> for
    // edges
    // this would still require that we maintain a map of name -> Item
    // in order to declare dependencies by name. as a result, when the Graph
    // returns there will be two Rc copies for every Item, so simply
    // drop() the map?
    // this would not be a problem if the Graph itself could take the name?
    // seems like a better design, since the Graph is the only thing that cares
    // about the name?
    // alternatively, instead of putting every single Item in the graph, we only
    // need to put in the names of the dependencies?
    //
    // push a map of name -> binding
    // add_node for every name
    // dependency reg. operates on name, e.g. "posts"
    // once the name topological order is computed,
    //   loop through each (name, binding) in the map
    //   and do output.extend(binding.items)

    // job should include an index as given by the graph order
    // the graph should operate entirely based on the indices

    match self.graph.resolve() {
      Ok(order) => {
        let mut swapped = 0u;

        println!("ordering: {}", order);
        println!("before: {}", self.jobs)

        for (to, from) in order.iter().enumerate() {
          let from = *from as uint;

          if to >= swapped && to != from {
            println!("swapping {} -> {}", from, to);
            self.jobs.as_mut_slice().swap(from, to);
          }

          println!("#{}: {}", swapped, self.jobs)
          swapped += 1;
        }

        println!("after: {}", self.jobs)
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
    let index = self.jobs.len() as i32;
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

  // gen.bind("posts", Match("posts/*.md"), posts_compiler, None)
  // gen.bind("post index", Create("something.html"), index_compiler, Some(["posts"]));
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

      // these two branches can't be DRYed because Rust
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

