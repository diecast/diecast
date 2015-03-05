//! Dependency tracking.

// FIXME: switch back to btreemap once this is fixed:
// https://github.com/rust-lang/rust/issues/22655

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use std::collections::btree_map::Keys;
use std::collections::btree_map::Entry::Vacant;

use std::hash::Hash;

use std::fmt;

use graphviz as dot;
use std::borrow::IntoCow;

/// Represents a dependency graph.
///
/// This graph tracks items and is able to produce an ordering
/// of the items that respects dependency constraints.
pub struct Graph<T> where T: Ord + Copy + Hash {
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

impl<T> Graph<T> where T: Ord + Copy + Hash {
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
        self.edges.entry(a).get()
            .unwrap_or_else(|v| v.insert(BTreeSet::new()))
            .insert(b);

        self.reverse.entry(b).get()
            .unwrap_or_else(|v| v.insert(BTreeSet::new()))
            .insert(a);
    }

    /// The nodes in the graph.
    pub fn nodes(&self) -> Keys<T, BTreeSet<T>> {
        self.edges.keys()
    }

    // TODO: this seems identical to the above?
    /// The dependents a node has.
    pub fn dependents_of(&self, node: T) -> Option<&BTreeSet<T>> {
        self.edges.get(&node)
    }

    pub fn dependencies_of(&self, node: T) -> Option<&BTreeSet<T>> {
        self.reverse.get(&node)
    }

    /// The number of dependencies a node has.
    pub fn dependency_count(&self, node: T) -> usize {
        self.reverse.get(&node).map(|s| s.len()).unwrap_or(0usize)
    }

    /// Topological ordering starting at the provided node.
    ///
    /// This essentially means: the given node plus all nodes
    /// that depend on it.
    pub fn resolve_only(&self, node: T) -> Result<VecDeque<T>, VecDeque<T>> {
        Topological::new(self).from(node)
    }

    /// Topological ordering of the entire graph.
    pub fn resolve(&self) -> Result<VecDeque<T>, VecDeque<T>> {
        Topological::new(self).all()
    }

    /// Render the dependency graph with graphviz. Visualize it with:
    ///
    /// ```bash
    /// $ dot -Tpng < deps.dot > deps.png && open deps.png
    /// ```
    pub fn render<W>(&self, output: &mut W)
    where W: Writer, T: Clone + fmt::Display {
        dot::render(self, output).unwrap()
    }
}

impl<T> fmt::Debug for Graph<T>
where T: fmt::Debug + Ord + Copy + Hash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(self.edges.fmt(f));
        Ok(())
    }
}

impl<'a, T> dot::Labeller<'a, T, (T, T)> for Graph<T> where T: Ord + Copy + Hash + fmt::Display {
    fn graph_id(&self) -> dot::Id<'a> {
        dot::Id::new("dependencies").unwrap()
    }

    fn node_id(&self, n: &T) -> dot::Id<'a> {
        dot::Id::new(format!("N{}", *n)).unwrap()
    }

    fn node_label(&self, n: &T) -> dot::LabelText {
        dot::LabelText::LabelStr(n.to_string().into_cow())
    }
}

impl<'a, T> dot::GraphWalk<'a, T, (T, T)> for Graph<T> where T: Ord + Clone + Copy + Hash {
    fn nodes(&self) -> dot::Nodes<'a, T> {
        Graph::<T>::nodes(self)
            .map(|n| *n)
            .collect::<Vec<T>>()
            .into_cow()
    }

    fn edges(&self) -> dot::Edges<'a, (T, T)> {
        let mut edges = Vec::new();

        for (&source, targets) in &self.edges {
            for &target in targets {
                edges.push((source, target));
            }
        }

        edges.into_cow()
    }

    fn source(&self, e: &(T, T)) -> T {
        let &(s, _) = e;
        return s;
    }

    fn target(&self, e: &(T, T)) -> T {
        let &(_, t) = e;
        return t;
    }
}

pub type Order<T> = VecDeque<T>;
pub type Cycle<T> = VecDeque<T>;

/// Encapsulates a topological sorting algorithm.
///
/// Performs a topological sorting of the provided graph
/// via a depth-first search. This ordering is such that
/// every node comes before the node(s) that depends on it.
struct Topological<'a, T: 'a> where T: Ord + Copy + Hash {
    /// The graph to traverse.
    graph: &'a Graph<T>,

    /// The nodes that have been visited so far
    visited: BTreeSet<T>,

    /// Nodes that are on the path to the current node.
    on_stack: BTreeSet<T>,

    /// Trace back a path in the case of a cycle.
    edge_to: BTreeMap<T, T>,
}

impl<'a, T: 'a> Topological<'a, T> where T: Ord + Copy + Hash {
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
    fn dfs(&mut self, node: T, out: &mut VecDeque<T>) -> Result<(), VecDeque<T>> {
        self.on_stack.insert(node);
        self.visited.insert(node);

        if let Some(neighbors) = self.graph.dependents_of(node) {
            for &neighbor in neighbors {
                // node isn't visited yet, so visit it
                // make sure to add a breadcrumb to trace our path
                // backwards in case there's a cycle
                if !self.visited.contains(&neighbor) {
                    self.edge_to.insert(neighbor, node);
                    try!(self.dfs(neighbor, out));
                }

                // cycle detected
                // trace back breadcrumbs to reconstruct the cycle's path
                else if self.on_stack.contains(&neighbor) {
                    let mut path = VecDeque::new();
                    path.push_front(neighbor);
                    path.push_front(node);

                    let mut previous = self.edge_to.get(&node);

                    while let Some(&found) = previous {
                        path.push_front(found);
                        previous = self.edge_to.get(&found);
                    }

                    return Err(path);
                }
            }
        }

        self.on_stack.remove(&node);
        out.push_front(node);
        Ok(())
    }

    /// recompile the dependencies of `node` and then `node` itself
    pub fn from(mut self, node: T) -> Result<Order<T>, Cycle<T>> {
        let mut order = VecDeque::new();

        try!(self.dfs(node, &mut order));

        Ok(order)
    }

    /// the typical resolution algorithm, returns a topological ordering
    /// of the nodes which honors the dependencies
    pub fn all(mut self) -> Result<Order<T>, Cycle<T>> {
        let mut order = VecDeque::new();

        for &node in self.graph.nodes() {
            if !self.visited.contains(&node) {
                try!(self.dfs(node, &mut order));
            }
        }

        Ok(order)
    }
}

#[cfg(test)]
mod test {
    use super::Graph;
    use std::old_io::File;

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

        let cycle = graph.resolve();

        assert!(cycle.is_err());
    }

    #[test]
    fn resolve_all() {
        let graph = helper_graph();

        let decomposed = graph.resolve();

        assert!(decomposed.is_ok());
    }

    #[test]
    fn resolve_only() {
        let graph = helper_graph();

        let resolve_single = graph.resolve_only(6);

        assert!(resolve_single.is_ok());
    }

    #[test]
    fn render() {
        use std::old_io::fs::{PathExtensions, unlink};

        let graph = helper_graph();

        let dot = Path::new("deps.dot");

        graph.render(&mut File::create(&dot));

        assert!(dot.exists());

        unlink(&dot).ok().expect("couldn't remove dot file");
    }
}
