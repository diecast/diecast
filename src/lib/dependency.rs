//! Dependency tracking.

use std::collections::{HashMap, HashSet, Deque};

use std::collections::hashmap::{Vacant, Occupied, SetItems, Keys};
use std::collections::ringbuf::RingBuf;

use graphviz as dot;
use graphviz::maybe_owned_vec::IntoMaybeOwnedVector;

use std::fmt::{mod, Show};
use std::hash::{hash, Hash};

use std::str;

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

  pub fn add_edge(&mut self, a: &'a T, b: &'a T) {
    match self.edges.entry(a) {
      Vacant(entry) => {
        let mut hs = HashSet::new();
        hs.insert(b);
        entry.set(hs);
      },
      Occupied(mut entry) => { entry.get_mut().insert(b); },
    };

    // store the other node as well
    match self.edges.entry(b) {
      Vacant(entry) => { entry.set(HashSet::new()); },
      Occupied(_) => (),
    }
  }

  pub fn nodes(&self) -> Keys<'a, &T, HashSet<&T>> {
    self.edges.keys()
  }

  pub fn neighbors_of(&self, node: &'a T) -> Option<SetItems<'a, &T>> {
    self.edges.find(&node).and_then(|s| {
      if !s.is_empty() {
        Some(s.iter())
      } else {
        None
      }
    })
  }

  // this node plus the ones that depend on this one
  pub fn resolve_only(&'a self, node: &'a T)
     -> Result<RingBuf<&'a T>, RingBuf<&'a T>> {
    let dfs = DFS::new(self);

    // topological order from the given node
    // (recompile its dependencies and then the node)
    dfs.topological_from(node)
  }

  pub fn resolve_all(&'a self) -> Result<RingBuf<&'a T>, RingBuf<&'a T>> {
    let dfs = DFS::new(self);
    dfs.topological()
  }

  /// $ dot -Tpng < deps.dot > deps.png && open deps.png
  pub fn render_dot<W>(&self, output: &mut W)
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

struct DFS<'b, T: 'b> {
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

impl<'b, T> DFS<'b, T>
  where T: Eq + Show + Hash {
  fn new(graph: &'b Graph<T>) -> DFS<'b, T> {
    DFS {
      graph: graph,
      visited: HashSet::new(),
      on_stack: HashSet::new(),
      edge_to: HashMap::new(),
      topological: RingBuf::new(),
      result: Ok(RingBuf::new()),
    }
  }

  fn dfs(&mut self, node: &'b T) {
    self.on_stack.insert(node);
    self.visited.insert(node);

    if let Some(mut neighbors) = self.graph.neighbors_of(node) {
      for &neighbor in neighbors {
        if self.result.is_err() {
          return;
        }

        else if !self.visited.contains(&neighbor) {
          self.edge_to.insert(neighbor, node);
          self.dfs(neighbor);
        }

        // cycle detected
        else if self.on_stack.contains(&neighbor) {
          let mut path: RingBuf<&T> = RingBuf::new();
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
  fn topological_from(mut self, node: &'b T)
     -> Result<RingBuf<&'b T>, RingBuf<&'b T>> {
    self.dfs(node);

    self.result.and(Ok(self.topological))
  }

  /// the typical resolution algorithm, returns a topological ordering
  /// of the nodes which honors the dependencies
  fn topological(mut self) -> Result<RingBuf<&'b T>, RingBuf<&'b T>> {
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

    let cycle = graph.resolve_all();

    println!("{}", cycle);

    assert!(cycle.is_err());
  }

  #[test]
  fn decompose_graph() {
    let a = &Item::new(Path::new("a"));
    let b = &Item::new(Path::new("b"));
    let c = &Item::new(Path::new("c"));

    let d = &Item::new(Path::new("d"));
    let e = &Item::new(Path::new("e"));

    let mut graph = Graph::new();

    // a -> b -> c
    graph.add_edge(a, b);
    graph.add_edge(b, c);

    // d -> e
    graph.add_edge(d, e);

    let decomposed = graph.resolve_all();

    println!("{}", decomposed);

    assert!(decomposed.is_ok());
  }

  #[test]
  fn topological_sort() {
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

    graph.render_dot(&mut File::create(&Path::new("deps.dot")));

    let decomposed = graph.resolve_all();

    println!("{}", decomposed);

    assert!(decomposed.is_ok());

    let resolve_single = graph.resolve_only(item6);

    println!("{}", resolve_single);

    assert!(resolve_single.is_ok());
  }
}
