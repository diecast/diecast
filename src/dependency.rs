//! Dependency tracking.

use std::collections::{HashMap, HashSet, RingBuf};

use std::collections::hash_map::Keys;
use std::collections::hash_map::Entry::Vacant;

use std::fmt;

use graphviz as dot;
use std::borrow::IntoCow;

/// Represents a dependency graph.
///
/// This graph tracks items and is able to produce an ordering
/// of the items that respects dependency constraints.
pub struct Graph {
    /// Edges in the graph; implicitly stores nodes.
    ///
    /// There's a key for every node in the graph, even if
    /// if it doesn't have any edges going out.
    edges: HashMap<usize, HashSet<usize>>,

    /// The dependencies a node has.
    ///
    /// This has to be stored separately because the direction of the
    /// edges represents dependency-respecting evaluation order,
    /// which is the reverse of the dependency relationship.
    ///
    /// e.g. the relationship that A depends on B can be represented as
    /// A -> B, so therefore the evaluation order which respects that
    /// dependency is the reverse, B -> A
    reverse: HashMap<usize, HashSet<usize>>,
}

impl Graph {
    pub fn new() -> Graph {
        Graph {
            edges: HashMap::new(),
            reverse: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, node: usize) {
        if let Vacant(entry) = self.edges.entry(node) {
            entry.insert(HashSet::new());
        }
    }

    /// Register a dependency constraint.
    pub fn add_edge(&mut self, a: usize, b: usize) {
        self.edges.entry(a).get()
            .unwrap_or_else(|v| v.insert(HashSet::new()))
            .insert(b);
        self.reverse.entry(b).get()
            .unwrap_or_else(|v| v.insert(HashSet::new()))
            .insert(a);
    }

    /// The nodes in the graph.
    pub fn nodes(&self) -> Keys<usize, HashSet<usize>> {
        self.edges.keys()
    }

    // TODO: this seems identical to the above?
    /// The dependents a node has.
    pub fn dependents_of(&self, node: usize) -> Option<&HashSet<usize>> {
        self.edges.get(&node)
    }

    pub fn dependencies_of(&self, node: usize) -> Option<&HashSet<usize>> {
        self.reverse.get(&node)
    }

    /// The number of dependencies a node has.
    pub fn dependency_count(&self, node: usize) -> usize {
        self.reverse.get(&node).map(|s| s.len()).unwrap_or(0us)
    }

    /// Topological ordering starting at the provided node.
    ///
    /// This essentially means: the given node plus all nodes
    /// that depend on it.
    pub fn resolve_only(&self, node: usize) -> Result<RingBuf<usize>, RingBuf<usize>> {
        Topological::new(self).from(node)
    }

    /// Topological ordering of the entire graph.
    pub fn resolve(&self) -> Result<RingBuf<usize>, RingBuf<usize>> {
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

impl fmt::Debug for Graph {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(self.edges.fmt(f));
        Ok(())
    }
}

/// A graph edge for graphviz
pub type Node = usize;
pub type Edge = (usize, usize);

impl<'a> dot::Labeller<'a, Node, Edge> for Graph {
    fn graph_id(&self) -> dot::Id<'a> {
        dot::Id::new("dependencies").unwrap()
    }

    fn node_id(&self, n: &Node) -> dot::Id<'a> {
        dot::Id::new(format!("N{}", *n)).unwrap()
    }

    fn node_label(&self, n: &Node) -> dot::LabelText {
        dot::LabelText::LabelStr(n.to_string().into_cow())
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

        for (&source, targets) in &self.edges {
            for &target in targets {
                edges.push((source, target));
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
    visited: HashSet<usize>,

    /// Nodes that are on the path to the current node.
    on_stack: HashSet<usize>,

    /// Trace back a path in the case of a cycle.
    edge_to: HashMap<usize, usize>,

    /// Nodes in an order which respects dependencies.
    topological: RingBuf<usize>,

    /// Either an ordering or the path of a cycle.
    result: Result<RingBuf<usize>, RingBuf<usize>>,
}

impl<'a> Topological<'a> {
    /// Construct the initial algorithm state.
    fn new(graph: &'a Graph) -> Topological<'a> {
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
    fn dfs(&mut self, node: usize) {
        self.on_stack.insert(node);
        self.visited.insert(node);

        if let Some(neighbors) = self.graph.dependents_of(node) {
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

                    let mut previous = self.edge_to.get(&node);

                    while let Some(&found) = previous {
                        path.push_front(found);
                        previous = self.edge_to.get(&found);
                    }

                    self.result = Err(path);
                }
            }
        }

        self.on_stack.remove(&node);
        self.topological.push_front(node);
    }

    /// recompile the dependencies of `node` and then `node` itself
    pub fn from(mut self, node: usize) -> Result<RingBuf<usize>, RingBuf<usize>> {
        self.dfs(node);

        self.result.and(Ok(self.topological))
    }

    /// the typical resolution algorithm, returns a topological ordering
    /// of the nodes which honors the dependencies
    pub fn all(mut self) -> Result<RingBuf<usize>, RingBuf<usize>> {
        trace!("graph: {:?}", self.graph);

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
    use super::Graph;
    use std::old_io::File;

    fn helper_graph() -> Graph {
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
