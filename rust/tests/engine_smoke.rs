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

/// End-to-end check that parameter updates routed to *non-Oscillator*
/// nodes actually take effect. The Output node is a GainNode with a
/// smoothed `target_gain`; setting GAIN_LEVEL=0 should silence the
/// stream after the smoother settles (~2 ms at 48 kHz). A previous
/// version that only mutated oscillator state would not exercise the
/// dispatch path for nodes whose set_param keys differ from the
/// oscillator's.
#[test]
fn output_node_gain_param_silences_stream() {
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
    let (nodes, edges, order) = graph.compile().expect("compile");

    let mut eng =
        AudioEngineWrapper::new(nodes, edges, order, 1, 48_000, 256, 0).expect("engine init");
    eng.set_param(0, param_hash::OSC_FREQUENCY, 880.0).expect("set freq");
    eng.start().expect("engine start");

    // ── Window 1: default Output gain (1.0) ─────────────────────────
    thread::sleep(Duration::from_millis(80));
    let ring = eng.output_ring();
    let mut buf1 = vec![0.0_f32; 8192];
    let n1 = ring.read(&mut buf1);
    assert!(n1 > 0, "first window produced no samples");
    let loud = buf1[..n1].iter().fold(0.0_f32, |acc, &s| acc.max(s.abs()));
    assert!(loud > 0.05, "expected audible output before gain cut; got {loud}");

    // ── Silence the Output GainNode (param routes by node_id=1) ─────
    eng.set_param(1, param_hash::GAIN_LEVEL, 0.0).expect("set output gain 0");

    // Discard the ~50 ms tail that crossfades from gain=1 to gain≈0
    // (smoothing constant 0.99 → 99% settled in ~460 samples ≈ 10 ms,
    // 99.9% in ~14 ms; 50 ms is a comfortable margin).
    thread::sleep(Duration::from_millis(60));
    let mut tail = vec![0.0_f32; 8192];
    let _ = ring.read(&mut tail);

    // ── Window 2: gain should be ~0 ─────────────────────────────────
    thread::sleep(Duration::from_millis(80));
    let mut buf2 = vec![0.0_f32; 8192];
    let n2 = ring.read(&mut buf2);
    assert!(n2 > 0, "second window produced no samples");
    let quiet = buf2[..n2].iter().fold(0.0_f32, |acc, &s| acc.max(s.abs()));
    assert!(quiet < loud * 0.05, "gain=0 did not silence stream; before={loud}, after={quiet}");

    eng.stop().expect("engine stop");
}

/// End-to-end check that FilterNode dispatch is wired correctly. Builds
/// Osc → Filter → Output; with the LP cutoff above the source frequency
/// the filter is transparent, then drops the cutoff well below the
/// source and asserts the throughput collapses. This exercises a third
/// distinct param-hash dispatch path (FILTER_CUTOFF) and the multi-hop
/// per-output-buffer routing introduced in loop 5.
#[test]
fn filter_node_cutoff_attenuates_high_frequency_source() {
    let mut graph = ShadowGraph::new(2);
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
            node_type: NodeType::Filter,
            num_inputs: 1,
            num_outputs: 1,
            parameters: HashMap::new(),
        })
        .expect("add filter");
    graph
        .add_node(Node {
            id: 2,
            node_type: NodeType::Output,
            num_inputs: 1,
            num_outputs: 0,
            parameters: HashMap::new(),
        })
        .expect("add output");
    graph
        .add_edge(Edge { from_node_id: 0, from_output_idx: 0, to_node_id: 1, to_input_idx: 0 })
        .expect("connect osc->filter");
    graph
        .add_edge(Edge { from_node_id: 1, from_output_idx: 0, to_node_id: 2, to_input_idx: 0 })
        .expect("connect filter->output");
    let (nodes, edges, order) = graph.compile().expect("compile");

    let mut eng =
        AudioEngineWrapper::new(nodes, edges, order, 2, 48_000, 256, 0).expect("engine init");

    // 8 kHz source — well-defined for both transparent and attenuated cases
    eng.set_param(0, param_hash::OSC_FREQUENCY, 8_000.0).expect("set osc freq");
    // Lowpass mode (Mode::LP = 0), cutoff well above source — transparent
    eng.set_param(1, param_hash::FILTER_MODE, 0.0).expect("set filter mode LP");
    eng.set_param(1, param_hash::FILTER_CUTOFF, 20_000.0).expect("set high cutoff");
    eng.start().expect("engine start");

    // ── Window 1: cutoff above source ───────────────────────────────
    thread::sleep(Duration::from_millis(120));
    let ring = eng.output_ring();
    let mut buf1 = vec![0.0_f32; 8192];
    let n1 = ring.read(&mut buf1);
    assert!(n1 > 0, "first window produced no samples");
    let pass = buf1[..n1].iter().fold(0.0_f32, |acc, &s| acc.max(s.abs()));
    assert!(pass > 0.05, "expected audible signal through transparent LP; got {pass}");

    // Drop cutoff to 100 Hz — 8 kHz source is now ~80× above cutoff
    eng.set_param(1, param_hash::FILTER_CUTOFF, 100.0).expect("set low cutoff");

    // FilterNode smooths cutoff once per block at 5% of the gap, so
    // convergence is ~0.107 s (1/(0.05 * 48000/256)). Wait ≥ 3× that
    // to land near steady state, then discard the transient ring tail.
    thread::sleep(Duration::from_millis(350));
    let mut tail = vec![0.0_f32; 16384];
    let _ = ring.read(&mut tail);

    // ── Window 2: cutoff far below source ───────────────────────────
    thread::sleep(Duration::from_millis(150));
    let mut buf2 = vec![0.0_f32; 8192];
    let n2 = ring.read(&mut buf2);
    assert!(n2 > 0, "second window produced no samples");
    let stop = buf2[..n2].iter().fold(0.0_f32, |acc, &s| acc.max(s.abs()));
    assert!(
        stop < pass * 0.25,
        "low-cutoff LP did not attenuate 8 kHz source enough; pass={pass}, stop={stop}"
    );

    eng.stop().expect("engine stop");
}
