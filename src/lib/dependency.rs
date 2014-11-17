//! Dependency tracking.

use std::collections::{HashMap, HashSet, RingBuf};

use std::collections::hash_map::{Vacant, Occupied, Keys};
use std::collections::hash_set::SetItems;

use graphviz as dot;
use graphviz::maybe_owned_vec::IntoMaybeOwnedVector;

use std::fmt::{mod, Show};
use std::hash::{hash, Hash};

use std::str;

/*
 * there should be support for dynamic dependencies.
 * dependencies shouldn't have to be registered beforehand.
 * instead, as items are retrieved they should be added to the
 * dependency graph, which should then be checked for consistency
 * to ensure that it doesn't conflict with the existing constraints:
 *
 *   * doesn't create a cycle
 *     assuming it didn't already have cycles, can this be optimized
 *     by running DFS from the new node? this would avoid having to
 *     re-run DFS on the entire graph
 *   * doesn't create a new dependency for something that was already built
 *     e.g. a new posts/post-blah.md when index.html, depending on
 *          posts/post-*.md, has already been built
 *     this means creating an edge to a node whose reference count is
 *     already 0
 *
 * dependent: has dependencies
 * dependencies: nodes required in order to build a dependent
 *
 * graph maintains two queues:
 *
 *   * `ready` queue of nodes that are ready to process
 *   * `waiting` queue of nodes whose dependencies aren't satisfied
 *     perhaps this should be a priority queue instead ordered
 *     by reference count, but then it doesn't handle updates of
 *     reference count easily
 *
 * these things need to occur given `A depends on B`:
 *
 *   * worker deques and processes `B` from `ready` queue
 *   * notify the graph that `B` finished, which decrements
 *     the reference count of `A` (and all neighbors of `B`)
 *   * `A`'s reference count reaches 0
 *   * graph moves the node from the `waiting` queue to the `ready` queue
 *   * repeat
 *
 * problems:
 *
 *   * how can the graph retain references to the nodes while
 *     also handing them off to workers to mutate? not possible afaik
 *
 *     maybe the graph should consist of Bindings and not Items?
 *     the graph would own Rc<Binding> and edges would be
 *     HashMap<Rc<Binding>, Weak<Binding>>.
 *
 */

/// Represents a dependency graph.
///
/// This graph tracks items and is able to produce an ordering
/// of the items that respects dependency constraints.
pub struct Graph<'a, T: 'a> {
  /// Edges in the graph; implicitly stores nodes.
  ///
  /// There's a key for every node in the graph, even if
  /// if it doesn't have any edges going out.
  edges: HashMap<&'a T, HashSet<&'a T>>,
}

impl<'a, T> Graph<'a, T>
  where T: Eq + Show + Hash {
  pub fn new() -> Graph<'a, T> {
    Graph {
      edges: HashMap::new(),
    }
  }

  pub fn add_node(&mut self, node: &'a T) {
    if let Vacant(entry) = self.edges.entry(node) {
      entry.set(HashSet::new());
    }
  }

  /// Register a dependency constraint.
  pub fn add_edge(&mut self, a: &'a T, b: &'a T) {
    match self.edges.entry(a) {
      Vacant(entry) => {
        let mut hs = HashSet::new();
        hs.insert(b);
        entry.set(hs);
      },
      Occupied(mut entry) => { entry.get_mut().insert(b); },
    };

    self.add_node(b);
  }

  /// The nodes in the graph.
  pub fn nodes(&self) -> Keys<'a, &T, HashSet<&T>> {
    self.edges.keys()
  }

  /// The neighbors of a given node.
  pub fn neighbors_of(&self, node: &'a T) -> Option<SetItems<'a, &T>> {
    self.edges.find(&node).and_then(|s| {
      if !s.is_empty() {
        Some(s.iter())
      } else {
        None
      }
    })
  }

  /// Topological ordering starting at the provided node.
  ///
  /// This essentially means: the given node plus all nodes
  /// that depend on it.
  pub fn resolve_only(&'a self, node: &'a T)
     -> Result<RingBuf<&'a T>, RingBuf<&'a T>> {
    Topological::new(self).from(node)
  }

  /// Topological ordering of the entire graph.
  pub fn resolve(&'a self) -> Result<RingBuf<&'a T>, RingBuf<&'a T>> {
    Topological::new(self).all()
  }

  /// Render the dependency graph with graphviz. Visualize it with:
  ///
  /// ```bash
  /// $ dot -Tpng < deps.dot > deps.png && open deps.png
  /// ```
  pub fn render<W>(&self, output: &mut W)
    where W: Writer {
    dot::render(self, output).unwrap()
  }
}

impl<'a, T> Show for Graph<'a, T>
  where T: Eq + Show + Hash {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    try!(self.edges.fmt(f));
    Ok(())
  }
}

/// A graph edge for graphviz
pub type Edge<'a, T> = (&'a T, &'a T);

impl<'a, T> dot::Labeller<'a, &'a T, Edge<'a, T>> for Graph<'a, T>
  where T: Eq + Hash + Show {
  fn graph_id(&'a self) -> dot::Id<'a> {
    dot::Id::new("dependencies")
  }

  fn node_id(&'a self, n: &&'a T) -> dot::Id<'a> {
    dot::Id::new(format!("N{}", hash(n)))
  }

  fn node_label(&'a self, n: &&'a T) -> dot::LabelText<'a> {
    dot::LabelStr(str::Owned(n.to_string()))
  }
}

impl<'a, T> dot::GraphWalk<'a, &'a T, Edge<'a, T>> for Graph<'a, T>
  where T: Eq + Hash + Show {
  fn nodes(&self) -> dot::Nodes<'a, &T> {
    self
      .nodes()
      .map(|n| *n)
      .collect::<Vec<&T>>()
      .into_maybe_owned()
  }

  fn edges(&'a self) -> dot::Edges<'a, Edge<T>> {
    let mut edges = Vec::new();

    for (&source, targets) in self.edges.iter() {
      for &target in targets.iter() {
        edges.push((source, target));
      }
    }

    edges.into_maybe_owned()
  }

  fn source(&self, e: &Edge<T>) -> &T {
    let &(s, _) = e;
    return s;
  }

  fn target(&self, e: &Edge<T>) -> &T {
    let &(_, t) = e;
    return t;
  }
}

/// Encapsulates a topological sorting algorithm.
///
/// Performs a topological sorting of the provided graph
/// via a depth-first search. This ordering is such that
/// every node comes before the node(s) that depends on it.
struct Topological<'b, T: 'b> {
  /// The graph to traverse.
  graph: &'b Graph<'b, T>,

  /// The nodes that have been visited so far
  visited: HashSet<&'b T>,

  /// Nodes that are on the path to the current node.
  on_stack: HashSet<&'b T>,

  /// Trace back a path in the case of a cycle.
  edge_to: HashMap<&'b T, &'b T>,

  /// Nodes in an order which respects dependencies.
  topological: RingBuf<&'b T>,

  /// Either an ordering or the path of a cycle.
  result: Result<RingBuf<&'b T>, RingBuf<&'b T>>,
}

impl<'b, T> Topological<'b, T>
  where T: Eq + Show + Hash {
  /// Construct the initial algorithm state.
  fn new(graph: &'b Graph<T>) -> Topological<'b, T> {
    Topological {
      graph: graph,
      visited: HashSet::new(),
      on_stack: HashSet::new(),
      edge_to: HashMap::new(),
      topological: RingBuf::new(),
      result: Ok(RingBuf::new()),
    }
  }

  /// Generate the topological ordering from a given node.
  ///
  /// This uses a recursive depth-first search, as it facilitates
  /// keeping track of a cycle, if any is present.
  fn dfs(&mut self, node: &'b T) {
    self.on_stack.insert(node);
    self.visited.insert(node);

    if let Some(mut neighbors) = self.graph.neighbors_of(node) {
      for &neighbor in neighbors {
        if self.result.is_err() {
          return;
        }

        // node isn't visited yet, so visit it
        // make sure to add a breadcrumb to trace our path
        // backwards in case there's a cycle
        else if !self.visited.contains(&neighbor) {
          self.edge_to.insert(neighbor, node);
          self.dfs(neighbor);
        }

        // cycle detected
        // trace back breadcrumbs to reconstruct the cycle's path
        else if self.on_stack.contains(&neighbor) {
          let mut path = RingBuf::new();
          path.push_front(neighbor);
          path.push_front(node);

          let mut previous = self.edge_to.find(&node);

          while let Some(&found) = previous {
            path.push_front(found);
            previous = self.edge_to.find(&found);
          }

          self.result = Err(path);
        }
      }
    }

    self.on_stack.remove(&node);
    self.topological.push_front(node);
  }

  /// recompile the dependencies of `node` and then `node` itself
  pub fn from(mut self, node: &'b T)
     -> Result<RingBuf<&'b T>, RingBuf<&'b T>> {
    self.dfs(node);

    self.result.and(Ok(self.topological))
  }

  /// the typical resolution algorithm, returns a topological ordering
  /// of the nodes which honors the dependencies
  pub fn all(mut self) -> Result<RingBuf<&'b T>, RingBuf<&'b T>> {
    for &node in self.graph.nodes() {
      if !self.visited.contains(&node) {
        self.dfs(node);
      }
    }

    self.result.and(Ok(self.topological))
  }
}

#[cfg(test)]
mod test {
  use item::Item;
  use super::Graph;
  use std::io::File;

  #[test]
  fn detect_cycles() {
    let a = &Item::new(Path::new("a"));
    let b = &Item::new(Path::new("b"));
    let c = &Item::new(Path::new("c"));

    let mut graph = Graph::new();
    graph.add_edge(a, b);
    graph.add_edge(b, c);
    graph.add_edge(c, a);

    let cycle = graph.resolve();

    assert!(cycle.is_err());
  }

  #[test]
  fn resolve_all() {
    let item0 = &Item::new(Path::new("0"));
    let item1 = &Item::new(Path::new("1"));
    let item2 = &Item::new(Path::new("2"));
    let item3 = &Item::new(Path::new("3"));
    let item4 = &Item::new(Path::new("4"));
    let item5 = &Item::new(Path::new("5"));
    let item6 = &Item::new(Path::new("6"));
    let item7 = &Item::new(Path::new("7"));
    let item8 = &Item::new(Path::new("8"));
    let item9 = &Item::new(Path::new("9"));
    let item10 = &Item::new(Path::new("10"));
    let item11 = &Item::new(Path::new("11"));
    let item12 = &Item::new(Path::new("12"));

    let mut graph = Graph::new();

    graph.add_edge(item8, item7);
    graph.add_edge(item7, item6);

    graph.add_edge(item6, item9);
    graph.add_edge(item9, item10);
    graph.add_edge(item9, item12);

    graph.add_edge(item9, item11);
    graph.add_edge(item11, item12);

    graph.add_edge(item6, item4);

    graph.add_edge(item0, item6);
    graph.add_edge(item0, item1);
    graph.add_edge(item0, item5);

    graph.add_edge(item5, item4);

    graph.add_edge(item2, item0);
    graph.add_edge(item2, item3);
    graph.add_edge(item3, item5);

    let decomposed = graph.resolve();

    assert!(decomposed.is_ok());
  }

  #[test]
  fn resolve_only() {
    let item0 = &Item::new(Path::new("0"));
    let item1 = &Item::new(Path::new("1"));
    let item2 = &Item::new(Path::new("2"));
    let item3 = &Item::new(Path::new("3"));
    let item4 = &Item::new(Path::new("4"));
    let item5 = &Item::new(Path::new("5"));
    let item6 = &Item::new(Path::new("6"));
    let item7 = &Item::new(Path::new("7"));
    let item8 = &Item::new(Path::new("8"));
    let item9 = &Item::new(Path::new("9"));
    let item10 = &Item::new(Path::new("10"));
    let item11 = &Item::new(Path::new("11"));
    let item12 = &Item::new(Path::new("12"));

    let mut graph = Graph::new();

    graph.add_edge(item8, item7);
    graph.add_edge(item7, item6);

    graph.add_edge(item6, item9);
    graph.add_edge(item9, item10);
    graph.add_edge(item9, item12);

    graph.add_edge(item9, item11);
    graph.add_edge(item11, item12);

    graph.add_edge(item6, item4);

    graph.add_edge(item0, item6);
    graph.add_edge(item0, item1);
    graph.add_edge(item0, item5);

    graph.add_edge(item5, item4);

    graph.add_edge(item2, item0);
    graph.add_edge(item2, item3);
    graph.add_edge(item3, item5);

    let resolve_single = graph.resolve_only(item6);

    assert!(resolve_single.is_ok());
  }

  #[test]
  fn render() {
    use std::io::fs::{PathExtensions, unlink};

    let item0 = &Item::new(Path::new("0"));
    let item1 = &Item::new(Path::new("1"));
    let item2 = &Item::new(Path::new("2"));
    let item3 = &Item::new(Path::new("3"));
    let item4 = &Item::new(Path::new("4"));
    let item5 = &Item::new(Path::new("5"));
    let item6 = &Item::new(Path::new("6"));
    let item7 = &Item::new(Path::new("7"));
    let item8 = &Item::new(Path::new("8"));
    let item9 = &Item::new(Path::new("9"));
    let item10 = &Item::new(Path::new("10"));
    let item11 = &Item::new(Path::new("11"));
    let item12 = &Item::new(Path::new("12"));

    let mut graph = Graph::new();

    graph.add_edge(item8, item7);
    graph.add_edge(item7, item6);

    graph.add_edge(item6, item9);
    graph.add_edge(item9, item10);
    graph.add_edge(item9, item12);

    graph.add_edge(item9, item11);
    graph.add_edge(item11, item12);

    graph.add_edge(item6, item4);

    graph.add_edge(item0, item6);
    graph.add_edge(item0, item1);
    graph.add_edge(item0, item5);

    graph.add_edge(item5, item4);

    graph.add_edge(item2, item0);
    graph.add_edge(item2, item3);
    graph.add_edge(item3, item5);

    let dot = Path::new("deps.dot");

    graph.render(&mut File::create(&dot));

    assert!(dot.exists());

    unlink(&dot).ok().expect("couldn't remove dot file");
  }
}
