//! Dependency tracking.

use std::collections::HashMap;
use std::collections::hashmap::{Vacant, Occupied};
use std::collections::hashmap::SetItems;

use std::collections::ringbuf::RingBuf;
use std::collections::Deque;

use std::collections::HashSet;

use std::fmt::{mod, Show};

use item::Item;

/// 1. x. build graph
/// 2. x. detect cycles
/// 3. x. separate connected components
/// 4. topological sort
pub struct Graph<'a> {
  /// Represents the graph itself
  edges: HashMap<&'a Item, HashSet<&'a Item>>,
}

impl<'a> Graph<'a> {
  fn new() -> Graph<'a> {
    Graph {
      edges: HashMap::new(),
    }
  }

  fn add_node(&mut self, item: &'a Item) {
    if let Vacant(entry) = self.edges.entry(item) {
      entry.set(HashSet::new());
    }
  }

  fn add_edge(&mut self, a: &'a Item, b: &'a Item) {
    if let Some(node) = self.edges.find_mut(&a) {
      node.insert(b);
    }
  }

  fn get_neighbors(&self, item: &'a Item) -> Option<SetItems<'a, &Item>> {
    self.edges.find(&item).map(|s| s.iter())
  }

  // decomposes into graph components
  // or returns a cycle on error
  fn decompose(&'a self) -> Result<Vec<Graph<'a>>, Vec<&'a Item>> {
    // represent the nodes to be considered
    let mut stack = Vec::new();

    // mark the visited nodes
    let mut visited = HashSet::new();

    // path to the current node
    let mut cycle_found = false;
    let mut path: Vec<&'a Item> = Vec::new();

    // the nodes in the graph
    let mut nodes = self.edges.keys().map(|k| *k);

    // map of node to component id
    let mut components = Vec::new();
    let mut component_id = 0;

    // perform DFS for every node, to ensure
    // that it is run for every component
    'detection: while let Some(root) = nodes.next() {
      // the node may have already been visited if it was part of a
      // component that was already DFSed
      if visited.contains(&root) {
        continue;
      }

      // add the node as the starting point for DFS
      stack.push(root);

      // create a new component
      components.push(Vec::new());

      // perform DFS
      while let Some(node) = stack.pop() {
        // cycle; the current node is already in the path
        if path.contains(&node) {
          cycle_found = true;
          break 'detection;
        }

        // add a breadcrumb to the path
        path.push(node);

        // set the node's component id
        components[component_id].push(node);

        // if it hasn't been visited, mark it as visited
        // then add all neighbors to be visited
        if !visited.contains(&node) {
          visited.insert(node);

          // add the neighbors to the DFS stack
          if let Some(neighbors) = self.get_neighbors(node) {
            stack.extend(neighbors.map(|n| *n));
          }

          // backtrack the path
          else {
            path.pop();
          }
        }
      }

      // starting on a new component, clear the path
      path.clear();

      // finished all nodes in this component
      component_id += 1;
    }

    // return the path if there was one
    if cycle_found {
      Err(path)
    }

    else {
      // TODO: Graph::new(self.edges.filter(|c| set.contains(c)))
      let graphs =
        components.iter().map(|c| {
          let mut g = Graph::new();

          for &node in c.iter() {
            g.add_node(node);

            if let Some(ref mut neighbors) = self.get_neighbors(node) {
              for &edge in neighbors {
                g.add_edge(node, edge);
              }
            }
          }

          return g;
        }).collect::<Vec<Graph>>();

      Ok(graphs)
    }
  }
}

impl<'a> Show for Graph<'a> {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    try!(self.edges.fmt(f));
    Ok(())
  }
}

#[cfg(test)]
mod test {
  pub use item::Item;
  pub use super::Graph;

  describe! dependency_graph {
    it "should detect cycles" {
      let a = &Item::new(Path::new("a"));
      let b = &Item::new(Path::new("b"));
      let c = &Item::new(Path::new("c"));

      let mut graph = Graph::new();
      graph.add_node(a);
      graph.add_node(b);
      graph.add_node(c);

      graph.add_edge(a, b);
      graph.add_edge(b, c);
      graph.add_edge(c, a);

      let cycle = graph.decompose();

      println!("{}", cycle);

      assert!(cycle.is_err());
    }

    it "should decompose the graph" {
      let a = &Item::new(Path::new("a"));
      let b = &Item::new(Path::new("b"));
      let c = &Item::new(Path::new("c"));

      let d = &Item::new(Path::new("d"));
      let e = &Item::new(Path::new("e"));

      let mut graph = Graph::new();
      graph.add_node(a);
      graph.add_node(b);
      graph.add_node(c);
      graph.add_node(d);
      graph.add_node(e);

      // a -> b -> c
      graph.add_edge(a, b);
      graph.add_edge(b, c);

      // d -> e
      graph.add_edge(d, e);

      let decomposed = graph.decompose();

      println!("{}", decomposed);

      assert!(decomposed.is_ok());
    }
  }
}
