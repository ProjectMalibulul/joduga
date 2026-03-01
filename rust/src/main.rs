/// Joduga Audio Synthesizer - Test Entry Point
///
/// This is a minimal test harness to validate the audio engine without the full Tauri UI.
/// It creates a simple audio graph (Oscillator -> Filter -> Gain -> Output) and processes it.
use joduga::{
    audio_engine_wrapper::AudioEngineWrapper,
    ffi::NodeType,
    shadow_graph::{Edge, Node, ShadowGraph},
};
use std::collections::HashMap;
use std::thread;
use std::time::Duration;

fn main() {
    println!("🎵 Joduga Audio Engine Test");
    println!("============================\n");

    // Create a shadow graph
    let mut graph = ShadowGraph::new(3);

    // Node 0: Oscillator (440 Hz sine wave)
    let osc_node = Node {
        id: 0,
        node_type: NodeType::Oscillator,
        num_inputs: 0,
        num_outputs: 1,
        parameters: HashMap::new(),
    };

    // Node 1: Low-pass filter (cutoff = 5000 Hz)
    let filter_node = Node {
        id: 1,
        node_type: NodeType::Filter,
        num_inputs: 1,
        num_outputs: 1,
        parameters: HashMap::new(),
    };

    // Node 2: Gain (volume = 0.5)
    let gain_node = Node {
        id: 2,
        node_type: NodeType::Gain,
        num_inputs: 1,
        num_outputs: 1,
        parameters: HashMap::new(),
    };

    // Node 3: Output (placeholder, not a real node type yet)
    let output_node = Node {
        id: 3,
        node_type: NodeType::Output,
        num_inputs: 1,
        num_outputs: 0,
        parameters: HashMap::new(),
    };

    // Add nodes
    graph.add_node(osc_node).expect("Failed to add oscillator");
    graph.add_node(filter_node).expect("Failed to add filter");
    graph.add_node(gain_node).expect("Failed to add gain");
    graph.add_node(output_node).expect("Failed to add output");

    // Wire them together: Osc -> Filter -> Gain -> Output
    graph
        .add_edge(Edge {
            from_node_id: 0,
            from_output_idx: 0,
            to_node_id: 1,
            to_input_idx: 0,
        })
        .expect("Failed to wire osc to filter");

    graph
        .add_edge(Edge {
            from_node_id: 1,
            from_output_idx: 0,
            to_node_id: 2,
            to_input_idx: 0,
        })
        .expect("Failed to wire filter to gain");

    graph
        .add_edge(Edge {
            from_node_id: 2,
            from_output_idx: 0,
            to_node_id: 3,
            to_input_idx: 0,
        })
        .expect("Failed to wire gain to output");

    // Validate and compile
    println!("✓ Graph created with 4 nodes and 3 edges");

    graph.validate().expect("Graph validation failed");
    println!("✓ Graph validated (no cycles detected)");

    let (nodes, edges, execution_order) = graph.compile().expect("Failed to compile graph");
    println!("✓ Graph compiled successfully");
    println!("  Execution order: {:?}", execution_order);

    // Initialize audio engine
    println!("\n🔊 Initializing audio engine...");
    let mut engine = AudioEngineWrapper::new(
        nodes,
        edges,
        execution_order,
        3,     // output node ID
        48000, // sample rate
        256,   // block size
        0,     // CPU core 0
    )
    .expect("Failed to initialize audio engine");

    println!("✓ Audio engine initialized");
    println!("  Sample rate: {} Hz", engine.sample_rate());
    println!("  Block size: {} samples", engine.block_size());

    // Start the audio engine
    engine.start().expect("Failed to start audio engine");
    println!("✓ Audio engine started (real-time thread running)");

    // Let it run for 5 seconds
    println!("\n⏳ Processing audio for 5 seconds...");
    thread::sleep(Duration::from_secs(1));

    // Test parameter changes
    println!("\n🎛️  Testing parameter updates:");

    // Change oscillator frequency to 880 Hz (A5)
    println!("  • Setting oscillator frequency to 880 Hz");
    engine
        .set_param(0, 0x811C9DC5, 880.0)
        .expect("Failed to set param");

    thread::sleep(Duration::from_secs(2));

    // Change filter cutoff to 2000 Hz
    println!("  • Setting filter cutoff to 2000 Hz");
    engine
        .set_param(1, 0x811C9DC5, 2000.0)
        .expect("Failed to set param");

    thread::sleep(Duration::from_secs(2));

    // Stop the engine
    println!("\n🛑 Stopping audio engine...");
    engine.stop().expect("Failed to stop audio engine");
    println!("✓ Audio engine stopped gracefully");

    println!("\n✅ Test completed successfully!");
}
