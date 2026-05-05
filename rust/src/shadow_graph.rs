/// Shadow graph — local representation of the audio graph maintained by Rust.
///
/// Used to validate the user-created graph (acyclicity, port bounds),
/// topologically sort it (Kahn's algorithm), and compile it into the C FFI
/// structures expected by the C++ engine.
///
/// # Example
///
/// ```
/// use joduga::shadow_graph::{Edge, Node, ShadowGraph};
/// use joduga::ffi::NodeType;
/// use std::collections::HashMap;
///
/// let mut g = ShadowGraph::new(/*output_node_id=*/ 1);
/// g.add_node(Node {
///     id: 0,
///     node_type: NodeType::Oscillator,
///     num_inputs: 0,
///     num_outputs: 1,
///     parameters: HashMap::new(),
/// })
/// .unwrap();
/// g.add_node(Node {
///     id: 1,
///     node_type: NodeType::Output,
///     num_inputs: 1,
///     num_outputs: 0,
///     parameters: HashMap::new(),
/// })
/// .unwrap();
/// g.add_edge(Edge {
///     from_node_id: 0,
///     from_output_idx: 0,
///     to_node_id: 1,
///     to_input_idx: 0,
/// })
/// .unwrap();
///
/// let (descs, conns, exec_order) = g.compile().unwrap();
/// assert_eq!(exec_order, vec![0, 1]);
/// assert_eq!(descs.len(), 2);
/// assert_eq!(conns.len(), 1);
/// ```
use crate::ffi::{NodeConnection, NodeDesc, NodeType};
use std::collections::{HashMap, VecDeque};

/// Maximum number of nodes allowed in a single graph.
pub const MAX_NODES: usize = 256;
/// Maximum number of edges allowed in a single graph.
pub const MAX_EDGES: usize = 1024;

#[derive(Debug, Clone)]
pub struct Node {
    pub id: u32,
    pub node_type: NodeType,
    pub num_inputs: u32,
    pub num_outputs: u32,
    pub parameters: HashMap<u32, f32>,
}

#[derive(Debug, Clone)]
pub struct Edge {
    pub from_node_id: u32,
    pub from_output_idx: u32,
    pub to_node_id: u32,
    pub to_input_idx: u32,
}

#[derive(Debug, Clone)]
pub struct ShadowGraph {
    pub nodes: HashMap<u32, Node>,
    pub edges: Vec<Edge>,
    pub output_node_id: u32,
}

impl ShadowGraph {
    pub fn new(output_node_id: u32) -> Self {
        Self { nodes: HashMap::new(), edges: Vec::new(), output_node_id }
    }

    pub fn add_node(&mut self, node: Node) -> Result<(), String> {
        if self.nodes.contains_key(&node.id) {
            return Err(format!("Node {} already exists", node.id));
        }
        if self.nodes.len() >= MAX_NODES {
            return Err(format!("Maximum node count ({MAX_NODES}) reached"));
        }
        self.nodes.insert(node.id, node);
        Ok(())
    }

    /// Remove a node and all edges connected to it.
    pub fn remove_node(&mut self, node_id: u32) -> Result<(), String> {
        if self.nodes.remove(&node_id).is_none() {
            return Err(format!("Node {} does not exist", node_id));
        }
        self.edges.retain(|e| e.from_node_id != node_id && e.to_node_id != node_id);
        Ok(())
    }

    pub fn add_edge(&mut self, edge: Edge) -> Result<(), String> {
        if self.edges.len() >= MAX_EDGES {
            return Err(format!("Maximum edge count ({MAX_EDGES}) reached"));
        }
        let from = self
            .nodes
            .get(&edge.from_node_id)
            .ok_or_else(|| format!("Source node {} does not exist", edge.from_node_id))?;
        if edge.from_output_idx >= from.num_outputs {
            return Err(format!(
                "Node {} has {} outputs, requested idx {}",
                edge.from_node_id, from.num_outputs, edge.from_output_idx
            ));
        }

        let to = self
            .nodes
            .get(&edge.to_node_id)
            .ok_or_else(|| format!("Target node {} does not exist", edge.to_node_id))?;
        if edge.to_input_idx >= to.num_inputs {
            return Err(format!(
                "Node {} has {} inputs, requested idx {}",
                edge.to_node_id, to.num_inputs, edge.to_input_idx
            ));
        }

        self.edges.push(edge);
        Ok(())
    }

    /// Remove an edge between two nodes.
    pub fn remove_edge(&mut self, from_node_id: u32, to_node_id: u32) -> Result<(), String> {
        let before = self.edges.len();
        self.edges.retain(|e| !(e.from_node_id == from_node_id && e.to_node_id == to_node_id));
        if self.edges.len() == before {
            return Err(format!("No edge from {} to {}", from_node_id, to_node_id));
        }
        Ok(())
    }

    /// Build the adjacency list (`from_node_id` → list of `to_node_id`) once.
    /// Shared by [`Self::validate`] and [`Self::topological_sort`] to avoid
    /// quadratic re-scans of `self.edges`.
    ///
    /// Each neighbour list is sorted by node ID so that downstream traversals
    /// (notably Kahn's algorithm) produce a deterministic order independent
    /// of `HashMap` iteration order.
    fn build_adjacency(&self) -> HashMap<u32, Vec<u32>> {
        let mut adj: HashMap<u32, Vec<u32>> = HashMap::with_capacity(self.nodes.len());
        for e in &self.edges {
            adj.entry(e.from_node_id).or_default().push(e.to_node_id);
        }
        for neighbours in adj.values_mut() {
            neighbours.sort_unstable();
        }
        adj
    }

    /// Validate acyclicity using DFS with proper HashMap-based colouring.
    pub fn validate(&self) -> Result<(), String> {
        let adj = self.build_adjacency();
        self.validate_with_adj(&adj)
    }

    fn validate_with_adj(&self, adj: &HashMap<u32, Vec<u32>>) -> Result<(), String> {
        // white = unvisited, grey = in recursion stack, black = done
        let mut color: HashMap<u32, u8> = self.nodes.keys().map(|&id| (id, 0u8)).collect();
        for &id in self.nodes.keys() {
            if color[&id] == 0 {
                Self::dfs_cycle(id, adj, &mut color)?;
            }
        }
        Ok(())
    }

    fn dfs_cycle(
        node: u32,
        adj: &HashMap<u32, Vec<u32>>,
        color: &mut HashMap<u32, u8>,
    ) -> Result<(), String> {
        color.insert(node, 1); // grey
        if let Some(neighbours) = adj.get(&node) {
            for &next in neighbours {
                match color.get(&next).copied().unwrap_or(0) {
                    1 => return Err("Graph contains a cycle".into()),
                    0 => Self::dfs_cycle(next, adj, color)?,
                    _ => {} // black — already finished
                }
            }
        }
        color.insert(node, 2); // black
        Ok(())
    }

    /// Kahn's algorithm — returns execution order as `Vec<u32>` of node IDs.
    ///
    /// Complexity: O(V + E). The adjacency list is built once and shared with
    /// the cycle-detection pass.
    pub fn topological_sort(&self) -> Result<Vec<u32>, String> {
        let adj = self.build_adjacency();
        self.validate_with_adj(&adj)?;
        self.topological_sort_with_adj(&adj)
    }

    fn topological_sort_with_adj(&self, adj: &HashMap<u32, Vec<u32>>) -> Result<Vec<u32>, String> {
        let mut in_degree: HashMap<u32, u32> = self.nodes.keys().map(|&id| (id, 0)).collect();
        for e in &self.edges {
            *in_degree.entry(e.to_node_id).or_default() += 1;
        }

        // Sort initial roots by ID so the output is deterministic across runs
        // (HashMap iteration order is randomized).
        let mut roots: Vec<u32> =
            in_degree.iter().filter(|(_, &d)| d == 0).map(|(&id, _)| id).collect();
        roots.sort_unstable();
        let mut queue: VecDeque<u32> = roots.into();

        let mut result = Vec::with_capacity(self.nodes.len());
        while let Some(id) = queue.pop_front() {
            result.push(id);
            if let Some(neighbours) = adj.get(&id) {
                for &next in neighbours {
                    let d = in_degree.get_mut(&next).unwrap();
                    *d -= 1;
                    if *d == 0 {
                        queue.push_back(next);
                    }
                }
            }
        }

        if result.len() != self.nodes.len() {
            return Err("Graph has cycles (topological sort incomplete)".into());
        }
        Ok(result)
    }

    /// Compile into the C FFI structures.
    #[allow(clippy::type_complexity)]
    pub fn compile(&self) -> Result<(Vec<NodeDesc>, Vec<NodeConnection>, Vec<u32>), String> {
        let adj = self.build_adjacency();
        self.validate_with_adj(&adj)?;
        let exec_order = self.topological_sort_with_adj(&adj)?;

        let node_descs: Vec<NodeDesc> = exec_order
            .iter()
            .map(|&id| {
                let n = &self.nodes[&id];
                NodeDesc {
                    node_id: id,
                    node_type: n.node_type,
                    num_inputs: n.num_inputs,
                    num_outputs: n.num_outputs,
                }
            })
            .collect();

        let connections: Vec<NodeConnection> = self
            .edges
            .iter()
            .map(|e| NodeConnection {
                from_node_id: e.from_node_id,
                from_output_idx: e.from_output_idx,
                to_node_id: e.to_node_id,
                to_input_idx: e.to_input_idx,
            })
            .collect();

        Ok((node_descs, connections, exec_order))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_node(id: u32, t: NodeType, ni: u32, no: u32) -> Node {
        Node { id, node_type: t, num_inputs: ni, num_outputs: no, parameters: HashMap::new() }
    }

    #[test]
    fn linear_chain() {
        let mut g = ShadowGraph::new(2);
        g.add_node(make_node(0, NodeType::Oscillator, 0, 1)).unwrap();
        g.add_node(make_node(1, NodeType::Filter, 1, 1)).unwrap();
        g.add_node(make_node(2, NodeType::Output, 1, 0)).unwrap();
        g.add_edge(Edge { from_node_id: 0, from_output_idx: 0, to_node_id: 1, to_input_idx: 0 })
            .unwrap();
        g.add_edge(Edge { from_node_id: 1, from_output_idx: 0, to_node_id: 2, to_input_idx: 0 })
            .unwrap();
        let order = g.topological_sort().unwrap();
        assert_eq!(order, vec![0, 1, 2]);
    }

    #[test]
    fn detect_cycle() {
        let mut g = ShadowGraph::new(0);
        g.add_node(make_node(0, NodeType::Filter, 1, 1)).unwrap();
        g.add_node(make_node(1, NodeType::Filter, 1, 1)).unwrap();
        g.add_edge(Edge { from_node_id: 0, from_output_idx: 0, to_node_id: 1, to_input_idx: 0 })
            .unwrap();
        g.add_edge(Edge { from_node_id: 1, from_output_idx: 0, to_node_id: 0, to_input_idx: 0 })
            .unwrap();
        assert!(g.validate().is_err());
    }

    #[test]
    fn non_contiguous_ids() {
        // IDs 10, 20, 30 — would crash the old vec-indexed cycle detection
        let mut g = ShadowGraph::new(30);
        g.add_node(make_node(10, NodeType::Oscillator, 0, 1)).unwrap();
        g.add_node(make_node(20, NodeType::Filter, 1, 1)).unwrap();
        g.add_node(make_node(30, NodeType::Output, 1, 0)).unwrap();
        g.add_edge(Edge { from_node_id: 10, from_output_idx: 0, to_node_id: 20, to_input_idx: 0 })
            .unwrap();
        g.add_edge(Edge { from_node_id: 20, from_output_idx: 0, to_node_id: 30, to_input_idx: 0 })
            .unwrap();
        assert!(g.validate().is_ok());
        let (_, _, order) = g.compile().unwrap();
        assert_eq!(order, vec![10, 20, 30]);
    }

    #[test]
    fn remove_node_and_edges() {
        let mut g = ShadowGraph::new(2);
        g.add_node(make_node(0, NodeType::Oscillator, 0, 1)).unwrap();
        g.add_node(make_node(1, NodeType::Filter, 1, 1)).unwrap();
        g.add_node(make_node(2, NodeType::Output, 1, 0)).unwrap();
        g.add_edge(Edge { from_node_id: 0, from_output_idx: 0, to_node_id: 1, to_input_idx: 0 })
            .unwrap();
        g.add_edge(Edge { from_node_id: 1, from_output_idx: 0, to_node_id: 2, to_input_idx: 0 })
            .unwrap();
        // Remove middle node — should remove connected edges
        g.remove_node(1).unwrap();
        assert_eq!(g.nodes.len(), 2);
        assert!(g.edges.is_empty());
    }

    #[test]
    fn remove_edge() {
        let mut g = ShadowGraph::new(1);
        g.add_node(make_node(0, NodeType::Oscillator, 0, 1)).unwrap();
        g.add_node(make_node(1, NodeType::Output, 1, 0)).unwrap();
        g.add_edge(Edge { from_node_id: 0, from_output_idx: 0, to_node_id: 1, to_input_idx: 0 })
            .unwrap();
        assert_eq!(g.edges.len(), 1);
        g.remove_edge(0, 1).unwrap();
        assert!(g.edges.is_empty());
    }

    #[test]
    fn duplicate_node_rejected() {
        let mut g = ShadowGraph::new(0);
        g.add_node(make_node(0, NodeType::Oscillator, 0, 1)).unwrap();
        assert!(g.add_node(make_node(0, NodeType::Filter, 1, 1)).is_err());
    }

    #[test]
    fn add_edge_unknown_source() {
        let mut g = ShadowGraph::new(1);
        g.add_node(make_node(1, NodeType::Output, 1, 0)).unwrap();
        let err = g
            .add_edge(Edge { from_node_id: 99, from_output_idx: 0, to_node_id: 1, to_input_idx: 0 })
            .unwrap_err();
        assert!(err.contains("Source node"));
    }

    #[test]
    fn add_edge_unknown_target() {
        let mut g = ShadowGraph::new(0);
        g.add_node(make_node(0, NodeType::Oscillator, 0, 1)).unwrap();
        let err = g
            .add_edge(Edge { from_node_id: 0, from_output_idx: 0, to_node_id: 99, to_input_idx: 0 })
            .unwrap_err();
        assert!(err.contains("Target node"));
    }

    #[test]
    fn add_edge_output_idx_out_of_bounds() {
        let mut g = ShadowGraph::new(1);
        g.add_node(make_node(0, NodeType::Oscillator, 0, 1)).unwrap();
        g.add_node(make_node(1, NodeType::Output, 1, 0)).unwrap();
        let err = g
            .add_edge(Edge { from_node_id: 0, from_output_idx: 5, to_node_id: 1, to_input_idx: 0 })
            .unwrap_err();
        assert!(err.contains("outputs"));
    }

    #[test]
    fn add_edge_input_idx_out_of_bounds() {
        let mut g = ShadowGraph::new(1);
        g.add_node(make_node(0, NodeType::Oscillator, 0, 1)).unwrap();
        g.add_node(make_node(1, NodeType::Output, 1, 0)).unwrap();
        let err = g
            .add_edge(Edge { from_node_id: 0, from_output_idx: 0, to_node_id: 1, to_input_idx: 5 })
            .unwrap_err();
        assert!(err.contains("inputs"));
    }

    #[test]
    fn remove_nonexistent_edge() {
        let mut g = ShadowGraph::new(1);
        g.add_node(make_node(0, NodeType::Oscillator, 0, 1)).unwrap();
        g.add_node(make_node(1, NodeType::Output, 1, 0)).unwrap();
        assert!(g.remove_edge(0, 1).is_err());
    }

    #[test]
    fn max_nodes_limit_enforced() {
        let mut g = ShadowGraph::new(0);
        for id in 0..MAX_NODES as u32 {
            g.add_node(make_node(id, NodeType::Filter, 1, 1)).unwrap();
        }
        let err = g.add_node(make_node(MAX_NODES as u32, NodeType::Filter, 1, 1)).unwrap_err();
        assert!(err.contains("Maximum node count"));
    }

    #[test]
    fn max_edges_limit_enforced() {
        let mut g = ShadowGraph::new(0);
        // Two nodes are enough; we just need MAX_EDGES + 1 add_edge calls.
        // A node with many input/output ports avoids port-bound failures.
        let ports = MAX_EDGES as u32 + 1;
        g.add_node(make_node(0, NodeType::Oscillator, 0, ports)).unwrap();
        g.add_node(make_node(1, NodeType::Output, ports, 0)).unwrap();
        for i in 0..MAX_EDGES as u32 {
            g.add_edge(Edge {
                from_node_id: 0,
                from_output_idx: i,
                to_node_id: 1,
                to_input_idx: i,
            })
            .unwrap();
        }
        let err = g
            .add_edge(Edge {
                from_node_id: 0,
                from_output_idx: MAX_EDGES as u32,
                to_node_id: 1,
                to_input_idx: MAX_EDGES as u32,
            })
            .unwrap_err();
        assert!(err.contains("Maximum edge count"));
    }

    #[test]
    fn topological_sort_is_deterministic_with_siblings() {
        // Two independent sources both feed a single sink. Without explicit
        // sorting, HashMap iteration order would make the relative position
        // of the sources non-deterministic across runs.
        let build = || {
            let mut g = ShadowGraph::new(99);
            g.add_node(make_node(10, NodeType::Oscillator, 0, 1)).unwrap();
            g.add_node(make_node(20, NodeType::Oscillator, 0, 1)).unwrap();
            g.add_node(make_node(30, NodeType::Oscillator, 0, 1)).unwrap();
            g.add_node(make_node(99, NodeType::Output, 3, 0)).unwrap();
            g.add_edge(Edge {
                from_node_id: 30,
                from_output_idx: 0,
                to_node_id: 99,
                to_input_idx: 0,
            })
            .unwrap();
            g.add_edge(Edge {
                from_node_id: 10,
                from_output_idx: 0,
                to_node_id: 99,
                to_input_idx: 1,
            })
            .unwrap();
            g.add_edge(Edge {
                from_node_id: 20,
                from_output_idx: 0,
                to_node_id: 99,
                to_input_idx: 2,
            })
            .unwrap();
            g
        };
        let first = build().topological_sort().unwrap();
        // Roots (10, 20, 30) must appear in ascending ID order, then sink 99.
        assert_eq!(first, vec![10, 20, 30, 99]);
        // Repeated invocations on freshly built graphs must agree.
        for _ in 0..32 {
            assert_eq!(build().topological_sort().unwrap(), first);
        }
    }

    /// Property-based check: for any randomly-generated DAG up to 32 nodes,
    /// `topological_sort` must produce a permutation of all node IDs in which
    /// every edge points strictly forward, and two consecutive calls on the
    /// same graph must produce the same order (determinism).
    ///
    /// Uses a tiny in-tree LCG so we don't pull in `rand` as a dependency.
    #[test]
    fn topological_sort_property_random_dags() {
        // Numerical Recipes LCG.
        struct Lcg(u64);
        impl Lcg {
            fn next_u32(&mut self) -> u32 {
                self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
                (self.0 >> 16) as u32
            }
            fn range(&mut self, n: u32) -> u32 {
                if n == 0 {
                    0
                } else {
                    self.next_u32() % n
                }
            }
        }

        for seed in 0u64..64 {
            let mut rng = Lcg(seed.wrapping_mul(2_654_435_761) ^ 0xDEAD_BEEF);
            let n_nodes: u32 = 2 + rng.range(31); // 2..=32 nodes
            let mut g = ShadowGraph::new(n_nodes - 1);

            // Each node has plenty of ports; the graph has at most 32 nodes
            // so 32 ports per side is always enough.
            for id in 0..n_nodes {
                g.add_node(make_node(id, NodeType::Filter, 32, 32)).unwrap();
            }

            // Generate a DAG by only emitting edges from lower→higher IDs.
            // Track which (to_node, to_input_idx) tuples are used to keep
            // edges semantically valid; multiple edges into the same
            // (node, input) would be fine for the topo property but make
            // the generator simpler.
            let n_edges = rng.range(n_nodes * 2);
            let mut used: std::collections::HashSet<(u32, u32)> = std::collections::HashSet::new();
            let mut next_in_idx: HashMap<u32, u32> = HashMap::new();
            for _ in 0..n_edges {
                let from = rng.range(n_nodes - 1);
                let to = from + 1 + rng.range(n_nodes - from - 1);
                let in_idx = *next_in_idx.entry(to).or_insert(0);
                if in_idx >= 32 || !used.insert((to, in_idx)) {
                    continue;
                }
                next_in_idx.insert(to, in_idx + 1);
                let out_idx = rng.range(32);
                g.add_edge(Edge {
                    from_node_id: from,
                    from_output_idx: out_idx,
                    to_node_id: to,
                    to_input_idx: in_idx,
                })
                .unwrap();
            }

            let order = g
                .topological_sort()
                .unwrap_or_else(|e| panic!("seed {seed}: topo sort failed: {e}"));

            // 1. Order is a permutation of all node IDs.
            assert_eq!(order.len(), n_nodes as usize, "seed {seed}: missing nodes");
            let mut sorted_order = order.clone();
            sorted_order.sort_unstable();
            let expected: Vec<u32> = (0..n_nodes).collect();
            assert_eq!(sorted_order, expected, "seed {seed}: not a permutation");

            // 2. Every edge points forward in the order.
            let pos: HashMap<u32, usize> =
                order.iter().enumerate().map(|(i, &id)| (id, i)).collect();
            for e in &g.edges {
                assert!(
                    pos[&e.from_node_id] < pos[&e.to_node_id],
                    "seed {seed}: edge {} -> {} violates topological order",
                    e.from_node_id,
                    e.to_node_id
                );
            }

            // 3. Determinism: a second call on the same graph yields the
            //    same order.
            let order2 = g.topological_sort().unwrap();
            assert_eq!(order, order2, "seed {seed}: non-deterministic order");
        }
    }

    #[test]
    fn remove_nonexistent_node() {
        let mut g = ShadowGraph::new(0);
        g.add_node(make_node(0, NodeType::Output, 0, 0)).unwrap();
        let err = g.remove_node(42).unwrap_err();
        assert!(err.contains("does not exist"));
    }

    #[test]
    fn compile_emits_topologically_ordered_descs() {
        // Build: 10 (osc) ─┐
        //                  ├─► 30 (filter) ─► 99 (output)
        // 20 (osc) ────────┘
        let mut g = ShadowGraph::new(99);
        g.add_node(make_node(10, NodeType::Oscillator, 0, 1)).unwrap();
        g.add_node(make_node(20, NodeType::Oscillator, 0, 1)).unwrap();
        g.add_node(make_node(30, NodeType::Filter, 2, 1)).unwrap();
        g.add_node(make_node(99, NodeType::Output, 1, 0)).unwrap();
        g.add_edge(Edge { from_node_id: 10, from_output_idx: 0, to_node_id: 30, to_input_idx: 0 })
            .unwrap();
        g.add_edge(Edge { from_node_id: 20, from_output_idx: 0, to_node_id: 30, to_input_idx: 1 })
            .unwrap();
        g.add_edge(Edge { from_node_id: 30, from_output_idx: 0, to_node_id: 99, to_input_idx: 0 })
            .unwrap();

        let (descs, conns, order) = g.compile().unwrap();

        // Order is deterministic (sources sorted by ID, then 30, then 99).
        assert_eq!(order, vec![10, 20, 30, 99]);
        assert_eq!(descs.len(), 4);
        // descs are emitted in exec_order — each desc's node_id must equal
        // the order entry at the same position.
        for (i, d) in descs.iter().enumerate() {
            assert_eq!(d.node_id, order[i], "desc[{i}] node_id mismatch");
        }
        // Per-node type passthrough.
        assert_eq!(descs[0].node_type, NodeType::Oscillator);
        assert_eq!(descs[2].node_type, NodeType::Filter);
        assert_eq!(descs[3].node_type, NodeType::Output);
        // All edges round-trip into connections (count + content).
        assert_eq!(conns.len(), 3);
        let edge_set: std::collections::HashSet<(u32, u32, u32, u32)> = conns
            .iter()
            .map(|c| (c.from_node_id, c.from_output_idx, c.to_node_id, c.to_input_idx))
            .collect();
        assert!(edge_set.contains(&(10, 0, 30, 0)));
        assert!(edge_set.contains(&(20, 0, 30, 1)));
        assert!(edge_set.contains(&(30, 0, 99, 0)));
    }

    #[test]
    fn compile_rejects_cycle() {
        let mut g = ShadowGraph::new(0);
        g.add_node(make_node(0, NodeType::Filter, 1, 1)).unwrap();
        g.add_node(make_node(1, NodeType::Filter, 1, 1)).unwrap();
        g.add_edge(Edge { from_node_id: 0, from_output_idx: 0, to_node_id: 1, to_input_idx: 0 })
            .unwrap();
        g.add_edge(Edge { from_node_id: 1, from_output_idx: 0, to_node_id: 0, to_input_idx: 0 })
            .unwrap();
        let err = g.compile().unwrap_err();
        assert!(err.to_lowercase().contains("cycle"), "expected cycle error, got: {err}");
    }
}
