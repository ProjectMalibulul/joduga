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

/// audio_engine_init must reject a CompiledGraph whose output_node_id
/// doesn't resolve to any node in the graph. Without this guard the
/// engine starts, the per-block ring-feed lookup silently fails, and
/// the host hears permanent silence.
///
/// We bypass ShadowGraph::compile() (which would also catch this on
/// the Rust side; see shadow_graph::validate_rejects_missing_output_node)
/// and call AudioEngineWrapper::new directly to exercise the C++ guard.
#[test]
fn cpp_init_rejects_unresolved_output_node_id() {
    let nodes = vec![joduga::ffi::NodeDesc {
        node_id: 0,
        node_type: NodeType::Oscillator,
        num_inputs: 0,
        num_outputs: 1,
    }];
    let edges: Vec<joduga::ffi::NodeConnection> = vec![];
    let order = vec![0u32];

    let res =
        AudioEngineWrapper::new(nodes, edges, order, /*output_node_id=*/ 99, 48_000, 256, 0);
    assert!(res.is_err(), "C++ init must reject output_node_id that doesn't resolve");
}

/// audio_engine_init must reject a config with block_size == 0; the
/// scratch buffers would be empty and every node's process() call
/// would still iterate over 0 frames, producing silence indefinitely.
#[test]
fn cpp_init_rejects_zero_block_size() {
    let nodes = vec![
        joduga::ffi::NodeDesc {
            node_id: 0,
            node_type: NodeType::Oscillator,
            num_inputs: 0,
            num_outputs: 1,
        },
        joduga::ffi::NodeDesc {
            node_id: 1,
            node_type: NodeType::Output,
            num_inputs: 1,
            num_outputs: 0,
        },
    ];
    let edges = vec![joduga::ffi::NodeConnection {
        from_node_id: 0,
        from_output_idx: 0,
        to_node_id: 1,
        to_input_idx: 0,
    }];
    let order = vec![0u32, 1u32];

    let res = AudioEngineWrapper::new(nodes, edges, order, 1, 48_000, /*block_size=*/ 0, 0);
    assert!(res.is_err(), "C++ init must reject block_size == 0");
}

/// status_register.cpu_load_permil must advance under a non-trivial
/// graph load. The C++ engine measures per-block processing time and
/// publishes (proc_ns * 1000 / block_ns) every block; a 4-node chain
/// (Osc → Filter → Reverb → Output) is heavy enough to land above the
/// per-mille rounding floor even on a fast CI runner.
#[test]
fn cpu_load_permil_advances_under_load() {
    fn make_node(id: u32, t: NodeType, inp: u32, out: u32) -> Node {
        Node { id, node_type: t, num_inputs: inp, num_outputs: out, parameters: HashMap::new() }
    }

    let mut graph = ShadowGraph::new(3);
    graph.add_node(make_node(0, NodeType::Oscillator, 0, 1)).unwrap();
    graph.add_node(make_node(1, NodeType::Filter, 1, 1)).unwrap();
    graph.add_node(make_node(2, NodeType::Reverb, 1, 1)).unwrap();
    graph.add_node(make_node(3, NodeType::Output, 1, 0)).unwrap();
    for (a, b) in [(0u32, 1u32), (1, 2), (2, 3)] {
        graph
            .add_edge(Edge { from_node_id: a, from_output_idx: 0, to_node_id: b, to_input_idx: 0 })
            .unwrap();
    }

    let (nodes, edges, order) = graph.compile().expect("compile heavy graph");
    let mut eng =
        AudioEngineWrapper::new(nodes, edges, order, 3, 48_000, 256, 0).expect("init heavy graph");
    eng.start().expect("start");

    // Give the engine enough wall time to populate at least a few
    // blocks (256 / 48k ≈ 5.3 ms each); 200 ms ≈ 37 blocks.
    thread::sleep(Duration::from_millis(200));

    let load = eng.cpu_load_permil();
    eng.stop().expect("stop");

    // Must be > 0 (engine actually measured something) and below the
    // 4000-permil cap that the C++ side clamps to.
    assert!(load > 0 && load < 4000, "cpu_load_permil out of expected range: {load}");
}

/// Calling audio_engine_start while the engine is already running
/// previously move-assigned a fresh std::thread over a joinable handle,
/// triggering std::terminate (i.e. an immediate process crash). The C
/// ABI now atomically transitions stopped→running and returns -2 if
/// the engine is already running. Verify via the Rust wrapper, which
/// surfaces the non-zero return code as an Err.
#[test]
fn double_start_is_safe_and_reports_error() {
    fn make_node(id: u32, t: NodeType, inp: u32, out: u32) -> Node {
        Node { id, node_type: t, num_inputs: inp, num_outputs: out, parameters: HashMap::new() }
    }
    let mut graph = ShadowGraph::new(1);
    graph.add_node(make_node(0, NodeType::Oscillator, 0, 1)).unwrap();
    graph.add_node(make_node(1, NodeType::Output, 1, 0)).unwrap();
    graph
        .add_edge(Edge { from_node_id: 0, from_output_idx: 0, to_node_id: 1, to_input_idx: 0 })
        .unwrap();
    let (nodes, edges, order) = graph.compile().expect("compile");
    let mut eng = AudioEngineWrapper::new(nodes, edges, order, 1, 48_000, 256, 0).expect("init");

    eng.start().expect("first start ok");
    let second = eng.start();
    assert!(second.is_err(), "second start must return Err, got {second:?}");

    // Stop must still succeed exactly once.
    eng.stop().expect("stop ok");
    // Second stop should be a successful no-op (idempotent).
    eng.stop().expect("idempotent stop");
}

/// graph_version must advance once the audio thread is running, and
/// must stop advancing once stop() returns. is_audio_thread_alive is
/// the primitive a host can use to detect a hung audio thread; this
/// test exercises both directions of the boolean.
#[test]
fn audio_thread_liveness_via_graph_version() {
    fn make_node(id: u32, t: NodeType, inp: u32, out: u32) -> Node {
        Node { id, node_type: t, num_inputs: inp, num_outputs: out, parameters: HashMap::new() }
    }
    let mut graph = ShadowGraph::new(1);
    graph.add_node(make_node(0, NodeType::Oscillator, 0, 1)).unwrap();
    graph.add_node(make_node(1, NodeType::Output, 1, 0)).unwrap();
    graph
        .add_edge(Edge { from_node_id: 0, from_output_idx: 0, to_node_id: 1, to_input_idx: 0 })
        .unwrap();
    let (nodes, edges, order) = graph.compile().expect("compile");
    let mut eng = AudioEngineWrapper::new(nodes, edges, order, 1, 48_000, 256, 0).expect("init");
    eng.start().expect("start");

    assert!(
        eng.is_audio_thread_alive(Duration::from_millis(50)),
        "graph_version did not advance during 50 ms while running"
    );

    eng.stop().expect("stop");

    // After stop, the counter must be frozen.
    let frozen = eng.graph_version();
    thread::sleep(Duration::from_millis(50));
    assert_eq!(eng.graph_version(), frozen, "graph_version advanced after stop()");
}

/// Regression test for an unclamped `FM_MOD_FREQ` blowing up the
/// oscillator's `mod_phase` accumulator. The FM/AM cases used a
/// single-step `if (mod_phase > TWO_PI) mod_phase -= TWO_PI` wrap, which
/// only normalises increments smaller than `TWO_PI` per sample. With a
/// huge mod-frequency the per-sample increment exceeds `TWO_PI` and
/// `mod_phase` grows unboundedly, eventually feeding garbage into
/// `std::sin` and producing NaN/Inf or denormal-shaped output.
///
/// Loop 23 fix: clamp `FM_MOD_FREQ`/`AM_MOD_FREQ` to the audible range
/// (mirrors `OSC_FREQUENCY`) and harden the wrap to a `while` loop.
#[test]
fn fm_oscillator_with_extreme_mod_freq_stays_bounded() {
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

    // FM waveform = 7. Carrier 440 Hz, mod depth 5.0 (radians of phase
    // modulation), and a *deliberately absurd* mod-freq that would
    // overrun a single-step phase wrap.
    eng.set_param(0, param_hash::WAVEFORM_TYPE, 7.0).expect("set waveform");
    eng.set_param(0, param_hash::OSC_FREQUENCY, 440.0).expect("set freq");
    eng.set_param(0, param_hash::FM_MOD_DEPTH, 5.0).expect("set mod depth");
    eng.set_param(0, param_hash::FM_MOD_FREQ, 1.0e9).expect("set mod freq");

    eng.start().expect("engine start");
    thread::sleep(Duration::from_millis(120));

    let ring = eng.output_ring();
    let mut buf = vec![0.0_f32; 8192];
    let n = ring.read(&mut buf);
    assert!(n > 0, "engine produced no FM samples");

    // Every sample must be finite and within [-1, 1] (sin output).
    let mut max_abs = 0.0_f32;
    for (i, &s) in buf[..n].iter().enumerate() {
        assert!(s.is_finite(), "sample {i} is non-finite ({s}) under extreme FM_MOD_FREQ");
        max_abs = max_abs.max(s.abs());
    }
    assert!(
        max_abs <= 1.0 + 1e-3,
        "FM output exceeded sine bound under extreme mod_freq: max |s| = {max_abs}"
    );

    eng.stop().expect("engine stop");
}

/// Loop 24 regression: SUPER_SAW oscillator with extreme `DETUNE` once
/// suffered the same single-step phase-wrap blowup as FM/AM (loop 23).
/// Per-voice phase advance is `TWO_PI * frequency * (1 + detune_amt) *
/// dt`; with unclamped detune the increment per sample could exceed
/// `TWO_PI`, leaving `saw_phases[j]` to grow without bound and
/// poisoning the saw output. Verify samples remain finite and bounded
/// after the clamp + while-wrap fix.
#[test]
fn super_saw_with_extreme_detune_stays_bounded() {
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

    eng.set_param(0, param_hash::WAVEFORM_TYPE, 11.0).expect("set super_saw");
    eng.set_param(0, param_hash::OSC_FREQUENCY, 20_000.0).expect("set freq");
    eng.set_param(0, param_hash::DETUNE, 1.0e6).expect("set extreme detune");

    eng.start().expect("engine start");
    thread::sleep(Duration::from_millis(120));

    let ring = eng.output_ring();
    let mut buf = vec![0.0_f32; 8192];
    let n = ring.read(&mut buf);
    assert!(n > 0, "engine produced no SUPER_SAW samples");

    let mut max_abs = 0.0_f32;
    for (i, &s) in buf[..n].iter().enumerate() {
        assert!(s.is_finite(), "sample {i} is non-finite ({s}) under extreme DETUNE");
        max_abs = max_abs.max(s.abs());
    }
    // SUPER_SAW averages voice outputs of `2*(p/TWO_PI) - 1` (a saw in
    // [-1, 1]); the average is also in [-1, 1] up to numerical slack.
    assert!(
        max_abs <= 1.0 + 1e-3,
        "SUPER_SAW exceeded saw bound under extreme detune: max |s| = {max_abs}"
    );

    eng.stop().expect("engine stop");
}

/// Loop 25 regression: Filter biquad's soft-clip used to update the
/// `z1`/`z2` state with the *unclipped* `y` before clamping the output
/// sample. Under high resonance an unstable pole pair would let state
/// grow without bound while the output looked clamped at ±4.0; once
/// state went non-finite, every subsequent sample was poisoned. The
/// fix reorders clip-then-state and adds a NaN-recovery scrub so a
/// single poisoned sample cannot lock the filter into permanent
/// silence-or-garbage.
///
/// This test drives an Osc → Filter → Output chain at maximum
/// resonance (Q=30) tuned to the carrier and asserts that every output
/// sample is finite and bounded by the soft-clip ceiling.
#[test]
fn filter_high_resonance_state_remains_bounded() {
    fn make_node(id: u32, t: NodeType, inp: u32, out: u32) -> Node {
        Node { id, node_type: t, num_inputs: inp, num_outputs: out, parameters: HashMap::new() }
    }

    let mut graph = ShadowGraph::new(2);
    graph.add_node(make_node(0, NodeType::Oscillator, 0, 1)).unwrap();
    graph.add_node(make_node(1, NodeType::Filter, 1, 1)).unwrap();
    graph.add_node(make_node(2, NodeType::Output, 1, 0)).unwrap();
    graph
        .add_edge(Edge { from_node_id: 0, from_output_idx: 0, to_node_id: 1, to_input_idx: 0 })
        .unwrap();
    graph
        .add_edge(Edge { from_node_id: 1, from_output_idx: 0, to_node_id: 2, to_input_idx: 0 })
        .unwrap();
    let (nodes, edges, order) = graph.compile().expect("compile");

    let mut eng = AudioEngineWrapper::new(nodes, edges, order, 2, 48_000, 256, 0).expect("init");

    // Sweep the filter to its resonance ceiling and tune cutoff to the
    // oscillator pitch so the unstable pole pair self-excites if state
    // is ever unbounded.
    eng.set_param(0, param_hash::OSC_FREQUENCY, 440.0).expect("set freq");
    eng.set_param(1, param_hash::FILTER_CUTOFF, 440.0).expect("set cutoff");
    eng.set_param(1, param_hash::FILTER_RESONANCE, 30.0).expect("set Q=30");

    eng.start().expect("start");
    thread::sleep(Duration::from_millis(120));

    let ring = eng.output_ring();
    let mut buf = vec![0.0_f32; 8192];
    let n = ring.read(&mut buf);
    assert!(n > 0, "no samples produced");

    let mut max_abs = 0.0_f32;
    for (i, &s) in buf[..n].iter().enumerate() {
        assert!(s.is_finite(), "sample {i} non-finite ({s}) under Q=30");
        max_abs = max_abs.max(s.abs());
    }
    // The biquad soft-clip caps |y| at 4.0; the output GainNode also
    // attenuates by its target_gain (default 1.0) so 4.0 is the upper
    // bound the test should ever observe.
    assert!(max_abs <= 4.0 + 1e-3, "filter output exceeded soft-clip ceiling: {max_abs}");

    eng.stop().expect("stop");
}

/// Loop 25 regression: `FILTER_MODE` was assigned via
/// `mode = static_cast<int>(value)` with no NaN guard. Per the C++
/// standard, casting a non-finite float to int is undefined behavior
/// and could yield any int value (including ones that, while never
/// matched in `compute_coefficients`'s switch, do trip its `default:`
/// arm in surprising ways). The fix rejects non-finite values and
/// clamps in-range integer modes; this test pins the no-crash
/// contract.
#[test]
fn filter_mode_rejects_nan_and_out_of_range() {
    fn make_node(id: u32, t: NodeType, inp: u32, out: u32) -> Node {
        Node { id, node_type: t, num_inputs: inp, num_outputs: out, parameters: HashMap::new() }
    }

    let mut graph = ShadowGraph::new(2);
    graph.add_node(make_node(0, NodeType::Oscillator, 0, 1)).unwrap();
    graph.add_node(make_node(1, NodeType::Filter, 1, 1)).unwrap();
    graph.add_node(make_node(2, NodeType::Output, 1, 0)).unwrap();
    graph
        .add_edge(Edge { from_node_id: 0, from_output_idx: 0, to_node_id: 1, to_input_idx: 0 })
        .unwrap();
    graph
        .add_edge(Edge { from_node_id: 1, from_output_idx: 0, to_node_id: 2, to_input_idx: 0 })
        .unwrap();
    let (nodes, edges, order) = graph.compile().expect("compile");

    let mut eng = AudioEngineWrapper::new(nodes, edges, order, 2, 48_000, 256, 0).expect("init");
    eng.set_param(0, param_hash::OSC_FREQUENCY, 440.0).expect("set freq");
    eng.set_param(1, param_hash::FILTER_MODE, f32::NAN).expect("set NaN mode");
    eng.set_param(1, param_hash::FILTER_MODE, 1.0e9).expect("set huge mode");

    eng.start().expect("start");
    thread::sleep(Duration::from_millis(120));

    let ring = eng.output_ring();
    let mut buf = vec![0.0_f32; 8192];
    let n = ring.read(&mut buf);
    assert!(n > 0, "no samples after rogue FILTER_MODE values");
    for (i, &s) in buf[..n].iter().enumerate() {
        assert!(s.is_finite(), "sample {i} non-finite ({s}) after rogue FILTER_MODE");
    }
    eng.stop().expect("stop");
}

/// Loop 26 regression: ReverbNode::set_param previously called
/// `lines[i].assign(n, 0.0f)` from inside the audio thread (via
/// apply_pending_params on every DELAY_TIME change). When the new size
/// exceeded capacity, std::vector reallocated under the global
/// allocator lock — a textbook real-time-discipline violation that
/// produces audible xruns under sustained automation.
///
/// The fix pre-allocates each delay line to MAX_DELAY_SAMPLES at
/// construction so DELAY_TIME changes are alloc-free. This test pins
/// the no-crash / bounded-output contract while hammering DELAY_TIME
/// from the host thread during live playback. (Asserting "no
/// allocation occurred" requires hooking the allocator and is out of
/// scope here; the structural invariant is verified by code review.)
#[test]
fn reverb_param_automation_under_load_stays_bounded() {
    fn make_node(id: u32, t: NodeType, inp: u32, out: u32) -> Node {
        Node { id, node_type: t, num_inputs: inp, num_outputs: out, parameters: HashMap::new() }
    }

    let mut graph = ShadowGraph::new(2);
    graph.add_node(make_node(0, NodeType::Oscillator, 0, 1)).unwrap();
    graph.add_node(make_node(1, NodeType::Reverb, 1, 1)).unwrap();
    graph.add_node(make_node(2, NodeType::Output, 1, 0)).unwrap();
    graph
        .add_edge(Edge { from_node_id: 0, from_output_idx: 0, to_node_id: 1, to_input_idx: 0 })
        .unwrap();
    graph
        .add_edge(Edge { from_node_id: 1, from_output_idx: 0, to_node_id: 2, to_input_idx: 0 })
        .unwrap();
    let (nodes, edges, order) = graph.compile().expect("compile");

    let mut eng = AudioEngineWrapper::new(nodes, edges, order, 2, 48_000, 256, 0).expect("init");
    eng.set_param(0, param_hash::OSC_FREQUENCY, 440.0).expect("set freq");
    eng.set_param(1, param_hash::DELAY_FEEDBACK, 0.9).expect("set fb");
    eng.set_param(1, param_hash::MIX, 0.5).expect("set mix");

    eng.start().expect("start");

    // Hammer DELAY_TIME changes while audio is running. Pre-fix this
    // would reallocate delay-line vectors on the audio thread.
    for step in 0..40 {
        let room = 0.1 + (step as f32 * 0.02) % 0.9;
        eng.set_param(1, param_hash::DELAY_TIME, room).expect("set room");
        thread::sleep(Duration::from_millis(5));
    }

    // Also send rogue non-finite values; set_param should quietly drop
    // them without disturbing state.
    eng.set_param(1, param_hash::DELAY_TIME, f32::NAN).expect("send NaN room");
    eng.set_param(1, param_hash::DELAY_FEEDBACK, f32::INFINITY).expect("send Inf fb");
    eng.set_param(1, param_hash::MIX, f32::NAN).expect("send NaN mix");

    thread::sleep(Duration::from_millis(60));

    let ring = eng.output_ring();
    let mut buf = vec![0.0_f32; 8192];
    let n = ring.read(&mut buf);
    assert!(n > 0, "no samples after reverb param storm");

    let mut max_abs = 0.0_f32;
    for (i, &s) in buf[..n].iter().enumerate() {
        assert!(s.is_finite(), "sample {i} non-finite ({s}) after reverb automation");
        max_abs = max_abs.max(s.abs());
    }
    // Reverb at fb=0.9 mix=0.5 with 440 Hz input should never approach
    // any large bound; 4.0 is generous.
    assert!(max_abs <= 4.0, "reverb output exceeded sane bound under param storm: {max_abs}");

    eng.stop().expect("stop");
}
