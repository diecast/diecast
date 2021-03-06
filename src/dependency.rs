//! Dependency tracking.

// FIXME: switch back to btreemap once this is fixed:
// https://github.com/rust-lang/rust/issues/22655

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::collections::btree_map::Keys;
use std::collections::btree_map::Entry::Vacant;
use std::borrow::Borrow;
use std::hash::Hash;
use std::any::Any;
use std::fmt;

// TODO: just make Graph use usize? or String?

/// Represents a dependency graph.
///
/// This graph tracks items and is able to produce an ordering
/// of the items that respects dependency constraints.
pub struct Graph<T>
where T: Ord + Clone + Hash {
    /// Edges in the graph; implicitly stores nodes.
    ///
    /// There's a key for every node in the graph, even if
    /// if it doesn't have any edges going out.
    edges: BTreeMap<T, BTreeSet<T>>,

    /// The dependencies a node has.
    ///
    /// This has to be stored separately because the direction of the
    /// edges represents dependency-respecting evaluation order,
    /// which is the reverse of the dependency relationship.
    ///
    /// e.g. the relationship that A depends on B can be represented as
    /// A -> B, so therefore the evaluation order which respects that
    /// dependency is the reverse, B -> A
    reverse: BTreeMap<T, BTreeSet<T>>,
}

impl<T> Graph<T>
where T: Ord + Clone + Hash {
    pub fn new() -> Graph<T> {
        Graph {
            edges: BTreeMap::new(),
            reverse: BTreeMap::new(),
        }
    }

    // TODO: is this even necessary? add_edge adds node if didn't exist
    // yes it is, because add_edge is explicitly for creating dependencies
    // even if something has no dependencies, we still want it in graph?
    pub fn add_node(&mut self, node: T) {
        if let Vacant(entry) = self.edges.entry(node) {
            entry.insert(BTreeSet::new());
        }
    }

    /// Register a dependency constraint.
    pub fn add_edge(&mut self, a: T, b: T) {
        self.edges.entry(a.clone())
            .or_insert(BTreeSet::new())
            .insert(b.clone());

        self.reverse.entry(b)
            .or_insert(BTreeSet::new())
            .insert(a);
    }

    /// The nodes in the graph.
    pub fn nodes(&self) -> Keys<T, BTreeSet<T>> {
        self.edges.keys()
    }

    // TODO: this seems identical to the above?
    /// The dependents a node has.
    pub fn dependents_of<Q: ?Sized>(&self, node: &Q) -> Option<&BTreeSet<T>>
    where T: Borrow<Q>, Q: Ord {
        self.edges.get(node)
    }

    // TODO: this and the above should just return an empty btreeset if no deps
    // can't cause it's a reference, argh
    pub fn dependencies_of<Q: ?Sized>(&self, node: &Q) -> Option<&BTreeSet<T>>
    where T: Borrow<Q>, Q: Ord {
        self.reverse.get(node)
    }

    /// The number of dependencies a node has.
    pub fn dependency_count<Q: ?Sized>(&self, node: &Q) -> usize
    where T: Borrow<Q>, Q: Ord {
        self.reverse.get(node).map_or(0usize, |s| s.len())
    }

    /// Topological ordering from a specific set of source nodes.
    #[allow(dead_code)]
    pub fn resolve(&self, nodes: Vec<T>) -> Result<Order<T>, CycleError<T>>
    where T: fmt::Debug + fmt::Display + Any {
        Topological::new(self).from(nodes)
    }

    /// Topological ordering of the entire graph.
    pub fn resolve_all(&self) -> Result<Order<T>, CycleError<T>>
    where T: fmt::Debug + fmt::Display + Any {
        Topological::new(self).all()
    }
}

impl<T> fmt::Debug for Graph<T>
where T: fmt::Debug + Ord + Clone + Hash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.edges.fmt(f)?;
        Ok(())
    }
}

pub type Order<T> = VecDeque<T>;

#[derive(Debug)]
pub struct CycleError<T>
where T: fmt::Debug + fmt::Display + Any {
    cycle: VecDeque<T>,
}

impl<T> fmt::Display for CycleError<T>
where T: fmt::Debug + fmt::Display + Any {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "dependency cycle detected:")?;

        for (idx, item) in self.cycle.iter().enumerate() {
            writeln!(f, "  {}. {}", idx + 1, item)?;
        }

        Ok(())
    }
}

impl<T> ::std::error::Error for CycleError<T>
where T: fmt::Debug + fmt::Display + Any {
    fn description(&self) -> &str {
        "dependency cycle detected"
    }
}

/// Encapsulates a topological sorting algorithm.
///
/// Performs a topological sorting of the provided graph
/// via a depth-first search. This ordering is such that
/// every node comes before the node(s) that depends on it.
struct Topological<'a, T: 'a>
where T: Ord + Clone + Hash {
    /// The graph to traverse.
    graph: &'a Graph<T>,

    /// The nodes that have been visited so far
    visited: BTreeSet<T>,

    /// Nodes that are on the path to the current node.
    on_stack: BTreeSet<T>,

    /// Trace back a path in the case of a cycle.
    edge_to: BTreeMap<T, T>,
}

impl<'a, T: 'a> Topological<'a, T>
where T: Ord + Clone + Hash {
    /// Construct the initial algorithm state.
    fn new(graph: &'a Graph<T>) -> Topological<'a, T> {
        Topological {
            graph: graph,
            visited: BTreeSet::new(),
            on_stack: BTreeSet::new(),
            edge_to: BTreeMap::new(),
        }
    }

    /// Generate the topological ordering from a given node.
    ///
    /// This uses a recursive depth-first search, as it facilitates
    /// keeping track of a cycle, if any is present.
    fn dfs(&mut self, node: T, out: &mut VecDeque<T>) -> Result<(), CycleError<T>>
    where T: fmt::Debug + fmt::Display + Any {
        self.on_stack.insert(node.clone());
        self.visited.insert(node.clone());

        if let Some(neighbors) = self.graph.dependents_of(&node) {
            for neighbor in neighbors {
                // node isn't visited yet, so visit it
                // make sure to add a breadcrumb to trace our path
                // backwards in case there's a cycle
                if !self.visited.contains(neighbor) {
                    self.edge_to.insert(neighbor.clone(), node.clone());
                    self.dfs(neighbor.clone(), out)?;
                }

                // cycle detected
                // trace back breadcrumbs to reconstruct the cycle's path
                else if self.on_stack.contains(&neighbor) {
                    let mut path = VecDeque::new();
                    path.push_front(neighbor.clone());
                    path.push_front(node.clone());

                    let mut previous = self.edge_to.get(&node);

                    while let Some(found) = previous {
                        path.push_front(found.clone());
                        previous = self.edge_to.get(&found);
                    }

                    return Err(CycleError { cycle: path });
                }
            }
        }

        self.on_stack.remove(&node);
        out.push_front(node);
        Ok(())
    }

    /// ordering from select nodes
    #[allow(dead_code)]
    pub fn from(mut self, nodes: Vec<T>) -> Result<Order<T>, CycleError<T>>
    where T: fmt::Display + fmt::Debug + Any {
        let mut order = VecDeque::new();

        for node in nodes {
            if !self.visited.contains(&node) {
                self.dfs(node, &mut order)?;
            }
        }

        Ok(order)
    }

    /// the typical resolution algorithm, returns a topological ordering
    /// of the nodes which honors the dependencies
    pub fn all(mut self) -> Result<Order<T>, CycleError<T>>
    where T: fmt::Display + fmt::Debug + Any {
        let mut order = VecDeque::new();

        for node in self.graph.nodes() {
            if !self.visited.contains(&node) {
                self.dfs(node.clone(), &mut order)?;
            }
        }

        Ok(order)
    }
}

#[cfg(test)]
mod test {
    use super::Graph;

    fn helper_graph() -> Graph<usize> {
        let mut graph = Graph::new();

        graph.add_edge(8, 7);
        graph.add_edge(7, 6);

        graph.add_edge(6, 9);
        graph.add_edge(9, 10);
        graph.add_edge(9, 12);

        graph.add_edge(9, 11);
        graph.add_edge(11, 12);

        graph.add_edge(6, 4);

        graph.add_edge(0, 6);
        graph.add_edge(0, 1);
        graph.add_edge(0, 5);

        graph.add_edge(5, 4);

        graph.add_edge(2, 0);
        graph.add_edge(2, 3);
        graph.add_edge(3, 5);

        return graph;
    }

    #[test]
    fn detect_cycles() {
        let mut graph = Graph::new();

        graph.add_edge(1, 2);
        graph.add_edge(2, 3);
        graph.add_edge(3, 1);

        let cycle = graph.resolve_all();

        assert!(cycle.is_err());
    }

    #[test]
    fn resolve_all() {
        let graph = helper_graph();

        let decomposed = graph.resolve_all();

        assert!(decomposed.is_ok());
    }

    #[test]
    fn resolve_only() {
        let graph = helper_graph();

        let resolve_single = graph.resolve(vec![6]);

        assert!(resolve_single.is_ok());
    }
}
