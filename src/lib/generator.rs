//! Site generation.

use sync::{Mutex, Arc};
use std::collections::RingBuf;
use std::collections::hash_map::{Vacant, Occupied};

use pattern::Pattern;
use compile::Compile;
use item::Item;
use dependency::Graph;

pub struct Job<C>
  where C: Compile + Send + Sync {
  id: i32,
  binding: &'static str,
  item: Item,
  compiler: C,
  dependencies: i32,
}

impl<C> for Job<C> where C: Compile {
  pub fn new(binding: &'static str, item: Item, compiler: C, id: i32) -> Job<C> {
    id: id,
    binding: binding,
    item: item,
    compiler: compiler,
    dependencies: 0,
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

  // /// The bindings
  // bindings: Vec<
  //             Binding<
  //               &'static str,
  //               BindAction<Box<Pattern>>,
  //               Box<Compiler>,
  //               Option<&'static [&'static str]>>>,

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

      jobs: Vec::new(),
      graph: Graph::new();
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
    //
  }

  // get the paths: Vec<Path> once
  // whenever something is bound, push a new Job for every match

  // I think the graph should construct the Jobs since it knows both the index and dependency count
  // I can't just set the dependency count to dependencies.len() in case the user specified
  // redundant or incorrect dependencies
  // to do this the graph can 

  fn add_job<C>(&mut self,
                binding: &'static str,
                path: Path,
                compiler: C,
                dependencies: Option<&'static [&'static str]>)
    where C: Compile {
    // make threadsafe? Mutex<jobs>?
    let index = self.jobs.len();
    self.jobs.push(Job::new(binding, Item::new(path), compiler, index));

    match self.items.entry(&name) {
      Vacant(entry) => entry.set(vec![index]),
      Occupied(mut entry) => entry.get_mut().push(index),
    }

    self.graph.add_node(index);

    if Some(deps) = dependencies {
      for dep in deps {
        for id in self.bindings[dep] {
          // id depends on index, so id must be done before index
          // so the edge goes: id -> index
          self.graph.add_edge(id, index);
        }
      }
    }
  }

  // gen.bind("posts", Match("posts/*.md"), posts_compiler, None)
  // gen.bind("post index", Create("something.html"), index_compiler, Some(["posts"]));
  pub fn bind<P, C>(mut self,
                    binding: &'static str,
                    action: Action<P>,
                    compiler: C,
                    dependencies: Option<&'static [&'static str]>)
                    -> Generator
    where P: Pattern + Send + Sync, C: Compile + Send + Sync {
      match action {
        Create(path) => {
          let target = self.output.join(path);
          self.add_job(binding, path, compiler, dependencies);
        },
        Match(pattern) => {
          let mut matched =
            self.paths.filter(|p| {
              let relative = &p.path_relative_from(&self.input).unwrap();
              pattern.matches(relative)
            });

          for path in matched {
            self.add_job(binding, path, compiler, dependencies);
          }
        },
      }

      self
  }
}

pub enum Action<P> where P: Pattern {
  Create(&'static str),
  Match(P)
}

struct Binding<P, C> where P: Pattern, C: Compile {
  name: &'static str,
  action: BindAction<P>,
  compiler: C,
  dependencies: Option<&'static [&'static str]>,
}

