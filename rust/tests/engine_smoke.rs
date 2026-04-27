//! End-to-end smoke test for the C++ audio engine via AudioEngineWrapper.
//!
//! Boots a minimal Oscillator → Output graph, runs the audio thread for
//! a short window without cpal, and asserts that the engine produced
//! non-zero samples into the output ring buffer. This is the only test
//! in the workspace that exercises the actual C++ DSP path; previous
//! coverage was only static (struct alignment, graph validation).
//!
//! The test deliberately does not assert `cpu_load_permil > 0` because
//! a 1-node graph is below the engine's measurement floor on fast CIs.

use joduga::audio_engine_wrapper::AudioEngineWrapper;
use joduga::ffi::NodeType;
use joduga::param_hash;
use joduga::shadow_graph::{Edge, Node, ShadowGraph};
use std::collections::HashMap;
use std::thread;
use std::time::Duration;

#[test]
fn engine_produces_non_zero_samples_for_oscillator_to_output() {
    // ── Build a 2-node graph: Osc (id=0) → Output (id=1) ─────────────
    let mut graph = ShadowGraph::new(1);
    graph
        .add_node(Node {
            id: 0,
            node_type: NodeType::Oscillator,
            num_inputs: 0,
            num_outputs: 1,
            parameters: HashMap::new(),
        })
        .expect("add osc");
    graph
        .add_node(Node {
            id: 1,
            node_type: NodeType::Output,
            num_inputs: 1,
            num_outputs: 0,
            parameters: HashMap::new(),
        })
        .expect("add output");
    graph
        .add_edge(Edge { from_node_id: 0, from_output_idx: 0, to_node_id: 1, to_input_idx: 0 })
        .expect("connect");
    graph.validate().expect("validate");
    let (nodes, edges, order) = graph.compile().expect("compile");

    // ── Boot wrapper (no cpal — engine writes directly to ring) ──────
    let mut eng = AudioEngineWrapper::new(
        nodes, edges, order, 1,      // output_node_id
        48_000, // sample rate
        256,    // block size
        0,      // cpu_core (best-effort hint; engine may ignore)
    )
    .expect("engine init");

    // 880 Hz sine: well above DC, well below Nyquist
    eng.set_param(0, param_hash::OSC_FREQUENCY, 880.0).expect("set freq");
    eng.start().expect("engine start");
    assert!(eng.is_running(), "engine should report running after start");

    // Let the audio thread produce ~20 blocks.
    thread::sleep(Duration::from_millis(120));

    // Drain whatever is in the ring and look for non-zero amplitude.
    let ring = eng.output_ring();
    let mut buf = vec![0.0_f32; 4096];
    let n_read = ring.read(&mut buf);
    assert!(n_read > 0, "engine produced no samples in 120 ms");

    let max_amp = buf[..n_read].iter().fold(0.0_f32, |acc, &s| acc.max(s.abs()));
    assert!(
        max_amp > 1e-4,
        "expected non-zero samples from oscillator; max abs amplitude = {max_amp}"
    );

    // Verify the parameter pipeline is alive — re-tuning the oscillator
    // should not return "queue full" or any other error.
    eng.set_param(0, param_hash::OSC_FREQUENCY, 220.0).expect("re-set freq");

    eng.stop().expect("engine stop");
    // Drop runs audio_engine_destroy.
}
