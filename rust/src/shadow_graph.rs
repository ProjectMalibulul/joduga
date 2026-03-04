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

    /// Validate acyclicity using DFS with proper HashMap-based colouring.
    pub fn validate(&self) -> Result<(), String> {
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
}
