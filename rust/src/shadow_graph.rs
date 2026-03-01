use crate::ffi::{NodeConnection, NodeDesc, NodeType};
/// Shadow graph representation in Rust.
/// This is a local representation of the audio graph that the Rust middleware
/// maintains to validate, topologically sort, and compile before sending to C++.
use std::collections::{HashMap, VecDeque};

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

/// The shadow graph—a local representation maintained by Rust
#[derive(Debug, Clone)]
pub struct ShadowGraph {
    pub nodes: HashMap<u32, Node>,
    pub edges: Vec<Edge>,
    pub output_node_id: u32,
}

impl ShadowGraph {
    pub fn new(output_node_id: u32) -> Self {
        ShadowGraph {
            nodes: HashMap::new(),
            edges: Vec::new(),
            output_node_id,
        }
    }

    /// Add a node to the graph
    pub fn add_node(&mut self, node: Node) -> Result<(), String> {
        if self.nodes.contains_key(&node.id) {
            return Err(format!("Node {} already exists", node.id));
        }
        self.nodes.insert(node.id, node);
        Ok(())
    }

    /// Add an edge between two nodes
    pub fn add_edge(&mut self, edge: Edge) -> Result<(), String> {
        // Validate that both nodes exist
        if !self.nodes.contains_key(&edge.from_node_id) {
            return Err(format!("Source node {} does not exist", edge.from_node_id));
        }
        if !self.nodes.contains_key(&edge.to_node_id) {
            return Err(format!("Target node {} does not exist", edge.to_node_id));
        }

        // Validate output/input indices
        let from_node = &self.nodes[&edge.from_node_id];
        if edge.from_output_idx >= from_node.num_outputs {
            return Err(format!(
                "Node {} has only {} outputs (requested {})",
                edge.from_node_id, from_node.num_outputs, edge.from_output_idx
            ));
        }

        let to_node = &self.nodes[&edge.to_node_id];
        if edge.to_input_idx >= to_node.num_inputs {
            return Err(format!(
                "Node {} has only {} inputs (requested {})",
                edge.to_node_id, to_node.num_inputs, edge.to_input_idx
            ));
        }

        self.edges.push(edge);
        Ok(())
    }

    /// Validate the graph (check for cycles and connectivity)
    pub fn validate(&self) -> Result<(), String> {
        // Check for cycles using DFS
        let mut visited = vec![false; self.nodes.len()];
        let mut rec_stack = vec![false; self.nodes.len()];

        for node_id in self.nodes.keys() {
            if !visited[*node_id as usize % self.nodes.len()] {
                if self.has_cycle(*node_id, &mut visited, &mut rec_stack)? {
                    return Err("Graph contains cycles".to_string());
                }
            }
        }

        Ok(())
    }

    /// Check if the graph has cycles (recursive helper)
    fn has_cycle(
        &self,
        node_id: u32,
        visited: &mut Vec<bool>,
        rec_stack: &mut Vec<bool>,
    ) -> Result<bool, String> {
        let idx = node_id as usize;
        visited[idx] = true;
        rec_stack[idx] = true;

        // Find all outgoing edges from this node
        for edge in &self.edges {
            if edge.from_node_id == node_id {
                let target_idx = edge.to_node_id as usize;

                if !visited[target_idx] {
                    if self.has_cycle(edge.to_node_id, visited, rec_stack)? {
                        return Ok(true);
                    }
                } else if rec_stack[target_idx] {
                    return Ok(true); // Back edge detected
                }
            }
        }

        rec_stack[idx] = false;
        Ok(false)
    }

    /// Topologically sort the graph (Kahn's algorithm)
    pub fn topological_sort(&self) -> Result<Vec<u32>, String> {
        // Validate first
        self.validate()?;

        // Build in-degree map
        let mut in_degree: HashMap<u32, u32> = HashMap::new();
        for node_id in self.nodes.keys() {
            in_degree.entry(*node_id).or_insert(0);
        }

        for edge in &self.edges {
            *in_degree.entry(edge.to_node_id).or_insert(0) += 1;
        }

        // Find nodes with in_degree 0
        let mut queue = VecDeque::new();
        for (node_id, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(*node_id);
            }
        }

        let mut result = Vec::new();
        let mut in_degree = in_degree;

        while let Some(node_id) = queue.pop_front() {
            result.push(node_id);

            // Process all edges from this node
            for edge in &self.edges {
                if edge.from_node_id == node_id {
                    let target = edge.to_node_id;
                    *in_degree.get_mut(&target).unwrap() -= 1;

                    if in_degree[&target] == 0 {
                        queue.push_back(target);
                    }
                }
            }
        }

        if result.len() != self.nodes.len() {
            return Err("Graph has cycles (topological sort incomplete)".to_string());
        }

        Ok(result)
    }

    /// Compile the graph into a format suitable for C++
    pub fn compile(&self) -> Result<(Vec<NodeDesc>, Vec<NodeConnection>, Vec<u32>), String> {
        // Get execution order
        let execution_order = self.topological_sort()?;

        // Convert nodes to NodeDesc
        let mut node_descs = Vec::new();
        let mut node_id_to_idx = HashMap::new();

        for (idx, &node_id) in execution_order.iter().enumerate() {
            node_id_to_idx.insert(node_id, idx as u32);
            let node = &self.nodes[&node_id];
            node_descs.push(NodeDesc {
                node_id,
                node_type: node.node_type,
                num_inputs: node.num_inputs,
                num_outputs: node.num_outputs,
            });
        }

        // Convert edges
        let node_connections = self
            .edges
            .iter()
            .map(|e| NodeConnection {
                from_node_id: e.from_node_id,
                from_output_idx: e.from_output_idx,
                to_node_id: e.to_node_id,
                to_input_idx: e.to_input_idx,
            })
            .collect();

        Ok((node_descs, node_connections, execution_order))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_validation() {
        let mut graph = ShadowGraph::new(1);

        let osc = Node {
            id: 0,
            node_type: NodeType::Oscillator,
            num_inputs: 0,
            num_outputs: 1,
            parameters: HashMap::new(),
        };

        let filter = Node {
            id: 1,
            node_type: NodeType::Filter,
            num_inputs: 1,
            num_outputs: 1,
            parameters: HashMap::new(),
        };

        graph.add_node(osc).unwrap();
        graph.add_node(filter).unwrap();

        let edge = Edge {
            from_node_id: 0,
            from_output_idx: 0,
            to_node_id: 1,
            to_input_idx: 0,
        };

        graph.add_edge(edge).unwrap();
        assert!(graph.validate().is_ok());
    }

    #[test]
    fn test_topological_sort() {
        let mut graph = ShadowGraph::new(2);

        for i in 0..3 {
            let node = Node {
                id: i,
                node_type: if i == 0 {
                    NodeType::Oscillator
                } else {
                    NodeType::Filter
                },
                num_inputs: if i == 0 { 0 } else { 1 },
                num_outputs: 1,
                parameters: HashMap::new(),
            };
            graph.add_node(node).unwrap();
        }

        // 0 -> 1 -> 2
        graph
            .add_edge(Edge {
                from_node_id: 0,
                from_output_idx: 0,
                to_node_id: 1,
                to_input_idx: 0,
            })
            .unwrap();
        graph
            .add_edge(Edge {
                from_node_id: 1,
                from_output_idx: 0,
                to_node_id: 2,
                to_input_idx: 0,
            })
            .unwrap();

        let order = graph.topological_sort().unwrap();
        assert_eq!(order, vec![0, 1, 2]);
    }
}
