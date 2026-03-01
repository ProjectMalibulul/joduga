//! Joduga — headless test harness.
//!
//! Builds a minimal graph (Osc → Filter → Gain → Output), starts the
//! engine for a few seconds, tweaks params, and shuts down.

use joduga::{
    audio_engine_wrapper::AudioEngineWrapper,
    ffi::NodeType,
    shadow_graph::{Edge, Node, ShadowGraph},
};
use std::collections::HashMap;
use std::thread;
use std::time::Duration;

fn main() {
    eprintln!("Joduga Audio Engine – test harness");
    eprintln!("===================================\n");

    // ── build shadow graph ──────────────────────────────────────────
    let mut graph = ShadowGraph::new(4);

    let nodes = [
        (0, NodeType::Oscillator, 0, 1),
        (1, NodeType::Filter, 1, 1),
        (2, NodeType::Gain, 1, 1),
        (3, NodeType::Output, 1, 0),
    ];
    for &(id, nt, ni, no) in &nodes {
        graph
            .add_node(Node {
                id,
                node_type: nt,
                num_inputs: ni,
                num_outputs: no,
                parameters: HashMap::new(),
            })
            .unwrap_or_else(|e| panic!("add_node({id}): {e}"));
    }

    let edges = [(0, 1), (1, 2), (2, 3)];
    for &(from, to) in &edges {
        graph
            .add_edge(Edge {
                from_node_id: from,
                from_output_idx: 0,
                to_node_id: to,
                to_input_idx: 0,
            })
            .unwrap_or_else(|e| panic!("add_edge({from}->{to}): {e}"));
    }

    graph.validate().expect("graph validation failed");
    let (compiled_nodes, compiled_edges, order) = graph.compile().expect("compile failed");
    eprintln!("graph compiled  order = {order:?}");

    // ── start engine ────────────────────────────────────────────────
    let mut engine = AudioEngineWrapper::new(
        compiled_nodes,
        compiled_edges,
        order,
        3,     // output node
        48000, // sample rate
        256,   // block size
        0,     // CPU core
    )
    .expect("engine init failed");

    engine.start().expect("engine start failed");
    eprintln!("engine running  sr={}  bs={}", engine.sample_rate(), engine.block_size());

    thread::sleep(Duration::from_secs(1));

    // tweak oscillator frequency → 880 Hz
    engine.set_param(0, 0x811C_9DC5, 880.0).expect("set_param osc freq");
    eprintln!("osc freq → 880 Hz");
    thread::sleep(Duration::from_secs(2));

    // tweak filter cutoff → 2000 Hz
    engine.set_param(1, 0x811C_9DC5, 2000.0).expect("set_param filter freq");
    eprintln!("filter cutoff → 2000 Hz");
    thread::sleep(Duration::from_secs(2));

    engine.stop().expect("engine stop failed");
    eprintln!("\ntest complete ✓");
}
