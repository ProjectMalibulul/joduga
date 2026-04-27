/// Shadow graph — local representation of the audio graph maintained by Rust.
///
/// Used to validate the user-created graph (acyclicity, port bounds),
/// topologically sort it (Kahn's algorithm), and compile it into the C FFI
/// structures expected by the C++ engine.
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

        // Reject exact duplicate edges. The C++ engine sums every connection
        // landing on a given input slot, so accepting the same edge twice
        // doubles that source's contribution silently. UI-side dedupe is not
        // guaranteed (drag-reconnect, JSON imports, etc.).
        if self.edges.iter().any(|e| {
            e.from_node_id == edge.from_node_id
                && e.from_output_idx == edge.from_output_idx
                && e.to_node_id == edge.to_node_id
                && e.to_input_idx == edge.to_input_idx
        }) {
            return Err(format!(
                "Duplicate edge from {}:{} to {}:{}",
                edge.from_node_id, edge.from_output_idx, edge.to_node_id, edge.to_input_idx
            ));
        }

        self.edges.push(edge);
        Ok(())
    }

    /// Remove a specific edge between two nodes and ports.
    pub fn remove_edge(
        &mut self,
        from_node_id: u32,
        from_output_idx: u32,
        to_node_id: u32,
        to_input_idx: u32,
    ) -> Result<(), String> {
        let before = self.edges.len();
        self.edges.retain(|e| {
            !(e.from_node_id == from_node_id
                && e.from_output_idx == from_output_idx
                && e.to_node_id == to_node_id
                && e.to_input_idx == to_input_idx)
        });
        if self.edges.len() == before {
            return Err(format!(
                "No edge from {}:{} to {}:{}",
                from_node_id, from_output_idx, to_node_id, to_input_idx
            ));
        }
        Ok(())
    }

    /// Validate the graph: output node existence and acyclicity.
    pub fn validate(&self) -> Result<(), String> {
        // The C++ engine resolves output_node_id at init; if the id is not in
        // the node map it sets output_feeder_slot = -1 and silently emits
        // silence. Catch that here so the user gets an explicit error
        // instead of a working "Play" button feeding nothing into the ring.
        if !self.nodes.contains_key(&self.output_node_id) {
            return Err(format!("Output node {} is not present in the graph", self.output_node_id));
        }

        // The id must also refer to an Output-type node. Otherwise the
        // C++ engine takes the wrong node's first output as the audio
        // sink: e.g. an Oscillator's raw waveform gets routed straight
        // to the speakers, bypassing every effect downstream.
        let out_node = &self.nodes[&self.output_node_id];
        if out_node.node_type != NodeType::Output {
            return Err(format!(
                "Node {} is configured as the audio output but has type {:?}, not Output",
                self.output_node_id, out_node.node_type
            ));
        }

        // ShadowGraph::{nodes, edges} are pub, so a caller can splice an
        // edge directly into `edges` bypassing add_edge's endpoint check.
        // Validate edge endpoints here so the cycle DFS below can rely on
        // the invariant that every adjacency neighbour is a known node.
        for e in &self.edges {
            if !self.nodes.contains_key(&e.from_node_id) {
                return Err(format!("Edge references missing source node {}", e.from_node_id));
            }
            if !self.nodes.contains_key(&e.to_node_id) {
                return Err(format!("Edge references missing target node {}", e.to_node_id));
            }
        }

        // The audio output node must have at least one incoming edge.
        // Without this the C++ engine's `output_feeder_buffer` falls
        // back to its sentinel and the ring-write block in
        // audio_engine.cpp is skipped — the engine starts, claims to
        // be running, and produces silence with no diagnostic. Catch
        // it here so the user gets a clear error at start time rather
        // than debugging a silent engine.
        let output_has_input = self.edges.iter().any(|e| e.to_node_id == self.output_node_id);
        if !output_has_input {
            return Err(format!(
                "Output node {} has no incoming edges — connect at least one source",
                self.output_node_id
            ));
        }

        // white = unvisited, grey = in recursion stack, black = done
        let mut color: HashMap<u32, u8> = self.nodes.keys().map(|&id| (id, 0u8)).collect();

        // Build adjacency list once
        let mut adj: HashMap<u32, Vec<u32>> = HashMap::new();
        for e in &self.edges {
            adj.entry(e.from_node_id).or_default().push(e.to_node_id);
        }

        for &id in self.nodes.keys() {
            if color[&id] == 0 {
                Self::dfs_cycle(id, &adj, &mut color)?;
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
                // Invariant: validate() guarantees every edge endpoint is a
                // known node, so `next` is always present in `color`.
                let c = *color.get(&next).expect(
                    "ShadowGraph invariant broken: dfs_cycle reached a node \
                     not in the color map (validate() should have caught this)",
                );
                match c {
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
    pub fn topological_sort(&self) -> Result<Vec<u32>, String> {
        self.validate()?;

        let mut in_degree: HashMap<u32, u32> = self.nodes.keys().map(|&id| (id, 0)).collect();
        for e in &self.edges {
            *in_degree.entry(e.to_node_id).or_default() += 1;
        }

        let mut queue: VecDeque<u32> =
            in_degree.iter().filter(|(_, &d)| d == 0).map(|(&id, _)| id).collect();

        let mut result = Vec::with_capacity(self.nodes.len());
        while let Some(id) = queue.pop_front() {
            result.push(id);
            for e in &self.edges {
                if e.from_node_id == id {
                    let d = in_degree.get_mut(&e.to_node_id).unwrap();
                    *d -= 1;
                    if *d == 0 {
                        queue.push_back(e.to_node_id);
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
        let exec_order = self.topological_sort()?;

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
        g.remove_edge(0, 0, 1, 0).unwrap();
        assert!(g.edges.is_empty());
    }

    #[test]
    fn duplicate_node_rejected() {
        let mut g = ShadowGraph::new(0);
        g.add_node(make_node(0, NodeType::Oscillator, 0, 1)).unwrap();
        assert!(g.add_node(make_node(0, NodeType::Filter, 1, 1)).is_err());
    }

    #[test]
    fn validate_rejects_missing_output_node() {
        // output_node_id = 99 but no such node exists.
        let mut g = ShadowGraph::new(99);
        g.add_node(make_node(0, NodeType::Oscillator, 0, 1)).unwrap();
        g.add_node(make_node(1, NodeType::Output, 1, 0)).unwrap();
        let err = g.validate().expect_err("missing output must fail");
        assert!(err.contains("Output node 99"), "unexpected error: {err}");
    }

    /// Edges spliced in directly (bypassing add_edge) that reference
    /// nodes which aren't in `nodes` must be rejected. Otherwise the
    /// cycle DFS would silently treat them as separate, and the C++
    /// engine would later be handed a connection list it cannot resolve.
    #[test]
    fn validate_rejects_edge_with_unknown_source_node() {
        let mut g = ShadowGraph::new(1);
        g.add_node(make_node(0, NodeType::Oscillator, 0, 1)).unwrap();
        g.add_node(make_node(1, NodeType::Output, 1, 0)).unwrap();
        // Splice past add_edge's validation:
        g.edges.push(Edge { from_node_id: 99, from_output_idx: 0, to_node_id: 1, to_input_idx: 0 });
        let err = g.validate().expect_err("unknown source must fail");
        assert!(err.contains("missing source node 99"), "unexpected error: {err}");
    }

    #[test]
    fn validate_rejects_edge_with_unknown_target_node() {
        let mut g = ShadowGraph::new(1);
        g.add_node(make_node(0, NodeType::Oscillator, 0, 1)).unwrap();
        g.add_node(make_node(1, NodeType::Output, 1, 0)).unwrap();
        g.edges.push(Edge { from_node_id: 0, from_output_idx: 0, to_node_id: 77, to_input_idx: 0 });
        let err = g.validate().expect_err("unknown target must fail");
        assert!(err.contains("missing target node 77"), "unexpected error: {err}");
    }

    /// validate() must reject a graph whose `output_node_id` points to a
    /// node of any type other than NodeType::Output. Otherwise the C++
    /// engine takes whatever's at slot 0 of that node as the audio sink,
    /// effectively routing e.g. the oscillator's raw waveform straight
    /// past every downstream effect.
    #[test]
    fn validate_rejects_non_output_typed_sink() {
        let mut g = ShadowGraph::new(0); // claims node 0 is the output
        g.add_node(make_node(0, NodeType::Oscillator, 0, 1)).unwrap();
        let err = g.validate().expect_err("non-Output sink must fail");
        assert!(err.contains("not Output"), "unexpected error: {err}");
    }

    #[test]
    fn validate_rejects_empty_graph() {
        // No nodes at all → output cannot exist.
        let g = ShadowGraph::new(0);
        assert!(g.validate().is_err());
    }

    #[test]
    fn compile_rejects_missing_output_node() {
        // compile() goes through validate(); the bad config must not reach the FFI.
        let mut g = ShadowGraph::new(42);
        g.add_node(make_node(0, NodeType::Oscillator, 0, 1)).unwrap();
        assert!(g.compile().is_err());
    }

    #[test]
    fn duplicate_edge_rejected() {
        // Two identical edges would be silently summed by the C++ engine,
        // doubling the source's contribution to the input slot.
        let mut g = ShadowGraph::new(1);
        g.add_node(make_node(0, NodeType::Oscillator, 0, 1)).unwrap();
        g.add_node(make_node(1, NodeType::Output, 1, 0)).unwrap();
        let e = Edge { from_node_id: 0, from_output_idx: 0, to_node_id: 1, to_input_idx: 0 };
        g.add_edge(e.clone()).unwrap();
        let err = g.add_edge(e).expect_err("duplicate must fail");
        assert!(err.contains("Duplicate edge"), "unexpected error: {err}");
        assert_eq!(g.edges.len(), 1);
    }

    #[test]
    fn parallel_edges_to_distinct_ports_are_allowed() {
        // Same source connecting to *different* input ports is legitimate
        // (e.g. routing one oscillator to L and R of a stereo node) and must
        // not be flagged as a duplicate.
        let mut g = ShadowGraph::new(1);
        g.add_node(make_node(0, NodeType::Oscillator, 0, 1)).unwrap();
        g.add_node(make_node(1, NodeType::Gain, 2, 1)).unwrap();
        g.add_edge(Edge { from_node_id: 0, from_output_idx: 0, to_node_id: 1, to_input_idx: 0 })
            .unwrap();
        g.add_edge(Edge { from_node_id: 0, from_output_idx: 0, to_node_id: 1, to_input_idx: 1 })
            .unwrap();
        assert_eq!(g.edges.len(), 2);
    }

    #[test]
    fn validate_rejects_disconnected_output() {
        // Output node typed correctly but no edge feeds into it. The
        // C++ engine would silently produce silence — catch at validate.
        let mut g = ShadowGraph::new(1);
        g.add_node(make_node(0, NodeType::Oscillator, 0, 1)).unwrap();
        g.add_node(make_node(1, NodeType::Output, 1, 0)).unwrap();
        let err = g.validate().expect_err("must fail with no edge to output");
        assert!(err.contains("no incoming edges"), "unexpected error: {err}");
    }
}
