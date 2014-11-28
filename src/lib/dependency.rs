//! Dependency tracking.

use std::collections::{HashMap, RingBuf, BitvSet};
use std::collections::hash_map::{Vacant, Occupied, Keys};
use std::collections::bitv_set::BitPositions;

use std::fmt::{mod, Show};

use graphviz as dot;

// TODO: compare BitvSet to HashSet
//       - usually better storage ?
//       - questionable performance? bit iteration vs indirection

/// Represents a dependency graph.
///
/// This graph tracks items and is able to produce an ordering
/// of the items that respects dependency constraints.
pub struct Graph {
  /// Edges in the graph; implicitly stores nodes.
  ///
  /// There's a key for every node in the graph, even if
  /// if it doesn't have any edges going out.
  edges: HashMap<u32, BitvSet>,

  /// The dependencies a node has.
  ///
  /// This has to be stored separately because the direction of the
  /// edges represents dependency-respecting evaluation order,
  /// which is the reverse of the dependency relationship.
  ///
  /// e.g. the relationship that A depends on B can be represented as
  /// A -> B, so therefore the evaluation order which respects that
  /// dependency is the reverse, B -> A
  reverse: HashMap<u32, BitvSet>,
}

impl Graph {
  pub fn new() -> Graph {
    Graph {
      edges: HashMap::new(),
      reverse: HashMap::new(),
    }
  }

  /// Register a dependency constraint.
  pub fn add_edge(&mut self, a: u32, b: u32) {
    match self.edges.entry(a) {
      Vacant(entry) => {
        let mut hs = BitvSet::new();
        hs.insert(b as uint);
        entry.set(hs);
      },
      Occupied(mut entry) => { entry.get_mut().insert(b as uint); },
    };

    // mirror the same thing for reverse
    match self.reverse.entry(b) {
      Vacant(entry) => {
        let mut hs = BitvSet::new();
        hs.insert(a as uint);
        entry.set(hs);
      },
      Occupied(mut entry) => { entry.get_mut().insert(a as uint); },
    };
  }

  /// The nodes in the graph.
  pub fn nodes(&self) -> Keys<u32, BitvSet> {
    self.edges.keys()
  }

  /// The neighbors of a given node.
  pub fn neighbors_of(&self, node: u32) -> Option<BitPositions> {
    self.edges.get(&node).and_then(|s| {
      if !s.is_empty() {
        Some(s.iter())
      } else {
        None
      }
    })
  }

  /// The dependents a node has.
  pub fn dependents_of(&self, node: u32) -> Option<&BitvSet> {
    self.edges.get(&node)
  }

  /// The number of dependencies a node has.
  pub fn dependency_count(&self, node: u32) -> u32 {
    self.reverse.get(&node).map(|s| s.len()).unwrap_or(0u) as u32
  }

  /// Topological ordering starting at the provided node.
  ///
  /// This essentially means: the given node plus all nodes
  /// that depend on it.
  pub fn resolve_only(&self, node: u32) -> Result<RingBuf<u32>, RingBuf<u32>> {
    Topological::new(self).from(node)
  }

  /// Topological ordering of the entire graph.
  pub fn resolve(&self) -> Result<RingBuf<u32>, RingBuf<u32>> {
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

impl Show for Graph {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    try!(self.edges.fmt(f));
    Ok(())
  }
}

/// A graph edge for graphviz
pub type Node = u32;
pub type Edge = (u32, u32);

impl<'a> dot::Labeller<'a, Node, Edge> for Graph {
  fn graph_id(&self) -> dot::Id<'a> {
    dot::Id::new("dependencies").unwrap()
  }

  fn node_id(&self, n: &Node) -> dot::Id<'a> {
    dot::Id::new(format!("N{}", *n)).unwrap()
  }

  fn node_label(&self, n: &Node) -> dot::LabelText {
    dot::LabelStr(n.to_string().into_cow())
  }
}

impl<'a> dot::GraphWalk<'a, Node, Edge> for Graph {
  fn nodes(&self) -> dot::Nodes<'a, Node> {
    self
      .nodes()
      .map(|n| *n)
      .collect::<Vec<Node>>()
      .into_cow()
  }

  fn edges(&self) -> dot::Edges<'a, Edge> {
    let mut edges = Vec::new();

    for (&source, targets) in self.edges.iter() {
      for target in targets.iter() {
        edges.push((source, target as u32));
      }
    }

    edges.into_cow()
  }

  fn source(&self, e: &Edge) -> Node {
    let &(s, _) = e;
    return s;
  }

  fn target(&self, e: &Edge) -> Node {
    let &(_, t) = e;
    return t;
  }
}

/// Encapsulates a topological sorting algorithm.
///
/// Performs a topological sorting of the provided graph
/// via a depth-first search. This ordering is such that
/// every node comes before the node(s) that depends on it.
struct Topological<'a> {
  /// The graph to traverse.
  graph: &'a Graph,

  /// The nodes that have been visited so far
  visited: BitvSet,

  /// Nodes that are on the path to the current node.
  on_stack: BitvSet,

  /// Trace back a path in the case of a cycle.
  edge_to: HashMap<u32, u32>,

  /// Nodes in an order which respects dependencies.
  topological: RingBuf<u32>,

  /// Either an ordering or the path of a cycle.
  result: Result<RingBuf<u32>, RingBuf<u32>>,
}

impl<'a> Topological<'a> {
  /// Construct the initial algorithm state.
  fn new(graph: &'a Graph) -> Topological<'a> {
    Topological {
      graph: graph,
      visited: BitvSet::new(),
      on_stack: BitvSet::new(),
      edge_to: HashMap::new(),
      topological: RingBuf::new(),
      result: Ok(RingBuf::new()),
    }
  }

  /// Generate the topological ordering from a given node.
  ///
  /// This uses a recursive depth-first search, as it facilitates
  /// keeping track of a cycle, if any is present.
  fn dfs(&mut self, node: u32) {
    self.on_stack.insert(node as uint);
    self.visited.insert(node as uint);

    if let Some(mut neighbors) = self.graph.neighbors_of(node) {
      for neighbor in neighbors {
        if self.result.is_err() {
          return;
        }

        // node isn't visited yet, so visit it
        // make sure to add a breadcrumb to trace our path
        // backwards in case there's a cycle
        else if !self.visited.contains(&neighbor) {
          self.edge_to.insert(neighbor as u32, node);
          self.dfs(neighbor as u32);
        }

        // cycle detected
        // trace back breadcrumbs to reconstruct the cycle's path
        else if self.on_stack.contains(&neighbor) {
          let mut path: RingBuf<u32> = RingBuf::new();
          path.push_front(neighbor as u32);
          path.push_front(node);

          let mut previous = self.edge_to.get(&node);

          while let Some(&found) = previous {
            path.push_front(found);
            previous = self.edge_to.get(&found);
          }

          self.result = Err(path);
        }
      }
    }

    self.on_stack.remove(&(node as uint));
    self.topological.push_front(node);
  }

  /// recompile the dependencies of `node` and then `node` itself
  pub fn from(mut self, node: u32) -> Result<RingBuf<u32>, RingBuf<u32>> {
    self.dfs(node);

    self.result.and(Ok(self.topological))
  }

  /// the typical resolution algorithm, returns a topological ordering
  /// of the nodes which honors the dependencies
  pub fn all(mut self) -> Result<RingBuf<u32>, RingBuf<u32>> {
    for &node in self.graph.nodes() {
      if !self.visited.contains(&(node as uint)) {
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
    let a = 1;
    let b = 2;
    let c = 3;

    let mut graph = Graph::new();
    graph.add_edge(a, b);
    graph.add_edge(b, c);
    graph.add_edge(c, a);

    let cycle = graph.resolve();

    assert!(cycle.is_err());
  }

  #[test]
  fn resolve_all() {
    let item0 = 0;
    let item1 = 1;
    let item2 = 2;
    let item3 = 3;
    let item4 = 4;
    let item5 = 5;
    let item6 = 6;
    let item7 = 7;
    let item8 = 8;
    let item9 = 9;
    let item10 = 10;
    let item11 = 11;
    let item12 = 12;

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
    let item0 = 0;
    let item1 = 1;
    let item2 = 2;
    let item3 = 3;
    let item4 = 4;
    let item5 = 5;
    let item6 = 6;
    let item7 = 7;
    let item8 = 8;
    let item9 = 9;
    let item10 = 10;
    let item11 = 11;
    let item12 = 12;

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

    let item0 = 0;
    let item1 = 1;
    let item2 = 2;
    let item3 = 3;
    let item4 = 4;
    let item5 = 5;
    let item6 = 6;
    let item7 = 7;
    let item8 = 8;
    let item9 = 9;
    let item10 = 10;
    let item11 = 11;
    let item12 = 12;

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
