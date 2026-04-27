//! Tauri backend for Joduga.
//!
//! Exposes `start_engine`, `stop_engine`, and `set_param` commands
//! to the React frontend via the Tauri IPC bridge.
//! Opens a cpal output stream so audio from the C++ engine ring buffer
//! is actually played through the system audio device.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use joduga::{
    audio_engine_wrapper::{AudioEngineWrapper, OutputRingBuffer},
    ffi::NodeType,
    shadow_graph::{Edge, Node, ShadowGraph},
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tauri::State;

/* -- types received from the frontend ----------------------- */

#[derive(Deserialize)]
pub struct ParamPair {
    pub hash: u32,
    pub value: f32,
}

#[derive(Deserialize)]
pub struct EngineNodeInfo {
    pub id: u32,
    pub engine_type: String,
    pub num_inputs: u32,
    pub num_outputs: u32,
    pub engine_subtype: u32,
    pub params: Vec<ParamPair>,
}

#[derive(Deserialize)]
pub struct EngineEdgeInfo {
    pub from_node: u32,
    pub from_port: u32,
    pub to_node: u32,
    pub to_port: u32,
}

/* -- managed state ------------------------------------------ */

/// EngineState holds the running engine behind an `RwLock`:
///   * start_engine / stop_engine take the write lock (mutate Option).
///   * set_param / get_engine_cpu_load_permil take the read lock —
///     the underlying AudioEngineWrapper enqueues into its own
///     lock-free param queue, so multiple concurrent UI-thread
///     callers do not need to serialize on each other. Previously a
///     single Mutex meant a UI knob storm (multi-touch, automation
///     bursts) serialized every set_param against every other one
///     and against the (much rarer) start/stop commands.
struct EngineState(RwLock<Option<RunningEngine>>);

/// Holds both the C++ engine wrapper and the cpal stream.
/// The stream must be kept alive for audio to play.
struct RunningEngine {
    wrapper: AudioEngineWrapper,
    _stream: cpal::Stream,
    /// Node ids that exist in the currently-running graph. set_param
    /// validates against this set so a stale id from a UI graph that
    /// has since been rebuilt produces a structured error instead of
    /// being silently dropped by the C++ param-drain (which iterates
    /// every node and matches on node_id, dropping unmatched cmds).
    valid_node_ids: std::collections::HashSet<u32>,
}

// cpal::Stream is !Send, but we only touch it behind an RwLock on the main thread
unsafe impl Send for RunningEngine {}
unsafe impl Sync for RunningEngine {}

/* -- helpers ------------------------------------------------ */

fn parse_engine_type(s: &str) -> Result<NodeType, String> {
    match s {
        "Oscillator" => Ok(NodeType::Oscillator),
        "Filter" => Ok(NodeType::Filter),
        "Gain" => Ok(NodeType::Gain),
        "Output" => Ok(NodeType::Output),
        "Delay" => Ok(NodeType::Delay),
        "Effects" => Ok(NodeType::Effects),
        "Reverb" => Ok(NodeType::Reverb),
        other => Err(format!("Unknown engine_type \"{other}\"")),
    }
}

/// Resolve which incoming `EngineNodeInfo` represents the audio Output.
///
/// Fails fast on missing or duplicate Output nodes — the previous
/// `unwrap_or_else(|| nodes.last()...)` fallback silently routed the
/// engine to whatever happened to be the last node in the array, which
/// after the loop-2 validate tightening would either crash with a
/// confusing "node not found" or worse, succeed with the wrong source.
fn resolve_output_node_id(nodes: &[EngineNodeInfo]) -> Result<u32, String> {
    let mut found: Option<u32> = None;
    for n in nodes {
        if n.engine_type == "Output" {
            if found.is_some() {
                return Err("Multiple Output nodes — keep only one".into());
            }
            found = Some(n.id);
        }
    }
    found.ok_or_else(|| "No Output node — add one to start the engine".into())
}

/// Open a cpal output stream that reads from the engine ring buffer.
///
/// The engine produces a single mono stream from its output node, but
/// cpal devices commonly require their *default* channel count
/// (typically stereo on desktop, sometimes 6/8 on pro audio rigs).
/// Hard-coding `channels: 1` previously caused either a build failure
/// on backends that reject non-default formats (WASAPI, ALSA) or, on
/// backends that *did* accept it, the callback called `ring.read` with
/// a `&mut [f32]` whose length was already in stereo frames — burning
/// through the engine's mono output at 2× speed and producing
/// underruns. Now we honour the device's default channel count and
/// fan the mono signal out across all channels.
fn open_cpal_stream(ring: Arc<OutputRingBuffer>, sample_rate: u32) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or("No audio output device found")?;
    let default_config = device
        .default_output_config()
        .map_err(|e| format!("query default cpal output config: {e}"))?;
    let channels: u16 = default_config.channels().max(1);
    let config = cpal::StreamConfig {
        channels,
        sample_rate: cpal::SampleRate(sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };
    eprintln!("[audio] cpal device default channels={channels}, sample_rate={sample_rate}");

    // Per-callback scratch for the mono read; we fan-out to N channels
    // when interleaving into the cpal buffer. Allocated lazily on the
    // realtime thread on first call to avoid allocating before
    // `build_output_stream` returns and to ensure the right size.
    // It's a Vec<f32> not an array because BufferSize::Default isn't
    // known at compile time.
    let mut mono_scratch: Vec<f32> = Vec::new();
    let ch = channels as usize;

    let stream = device
        .build_output_stream(
            &config,
            move |buffer: &mut [f32], _: &cpal::OutputCallbackInfo| {
                // cpal hands us interleaved frames: buffer.len() = frames * channels.
                let frames = buffer.len() / ch.max(1);
                if mono_scratch.len() < frames {
                    mono_scratch.resize(frames, 0.0);
                }
                let mono = &mut mono_scratch[..frames];
                let n = ring.read(mono);
                // Fan-out: write mono[f] to every channel of frame f.
                for f in 0..n {
                    let s = mono[f];
                    for c in 0..ch {
                        buffer[f * ch + c] = s;
                    }
                }
                // Underrun: zero the rest.
                for sample in &mut buffer[n * ch..] {
                    *sample = 0.0;
                }
            },
            |err| eprintln!("cpal error: {err}"),
            None,
        )
        .map_err(|e| format!("{e}"))?;
    stream.play().map_err(|e| format!("{e}"))?;
    Ok(stream)
}

/* -- commands ----------------------------------------------- */

#[tauri::command]
fn start_engine(
    nodes: Vec<EngineNodeInfo>,
    edges: Vec<EngineEdgeInfo>,
    state: State<'_, EngineState>,
) -> Result<(), String> {
    let mut guard = state.0.write().map_err(|e| e.to_string())?;

    // Stop previous engine if running
    if let Some(ref mut eng) = *guard {
        let _ = eng.wrapper.stop();
    }
    *guard = None;

    // Find output node (fail fast on missing/duplicate)
    let output_id = resolve_output_node_id(&nodes)?;

    // Build shadow graph
    let mut graph = ShadowGraph::new(output_id);

    for n in &nodes {
        graph
            .add_node(Node {
                id: n.id,
                node_type: parse_engine_type(&n.engine_type)?,
                num_inputs: n.num_inputs,
                num_outputs: n.num_outputs,
                parameters: HashMap::new(),
            })
            .map_err(|e| e.to_string())?;
    }

    for e in &edges {
        graph
            .add_edge(Edge {
                from_node_id: e.from_node,
                from_output_idx: e.from_port,
                to_node_id: e.to_node,
                to_input_idx: e.to_port,
            })
            .map_err(|e| e.to_string())?;
    }

    graph.validate().map_err(|e| e.to_string())?;
    let (compiled_nodes, compiled_edges, order) = graph.compile().map_err(|e| e.to_string())?;

    let sample_rate = 48000_u32;

    let mut engine = AudioEngineWrapper::new(
        compiled_nodes,
        compiled_edges,
        order,
        output_id,
        sample_rate,
        256,
        0,
    )?;

    // Send initial param values. Previously we used `let _ = ...` to
    // discard errors; that silently dropped knob updates if the param
    // queue ever filled (cap 8192, drained by the audio thread which
    // has not started yet on this code path) or if the FFI call
    // failed for any reason. With graphs pushing >1k initial params
    // the queue can come close to capacity, and a silent drop here
    // means the C++ node is left running with whatever its compile-
    // time defaults were. Surface the failure instead.
    for n in &nodes {
        for p in &n.params {
            engine.set_param(n.id, p.hash, p.value).map_err(|e| {
                format!("set_param failed for node {} hash 0x{:x}: {e}", n.id, p.hash)
            })?;
        }
        // Send mode-select param so the C++ node initialises to the right subtype
        let mode_hash: Option<u32> = match n.engine_type.as_str() {
            "Oscillator" => Some(joduga::param_hash::WAVEFORM_TYPE),
            "Filter" => Some(joduga::param_hash::FILTER_MODE),
            "Gain" => Some(joduga::param_hash::GAIN_MODE),
            "Delay" => Some(joduga::param_hash::DELAY_MODE),
            "Effects" => Some(joduga::param_hash::EFFECTS_MODE),
            _ => None,
        };
        if let Some(h) = mode_hash {
            engine.set_param(n.id, h, n.engine_subtype as f32).map_err(|e| {
                format!("set_param (mode) failed for node {} hash 0x{:x}: {e}", n.id, h)
            })?;
        }
    }

    // Open cpal output stream BEFORE starting engine
    let ring = engine.output_ring();
    let stream = open_cpal_stream(ring, sample_rate)?;

    engine.start()?;
    let valid_node_ids: std::collections::HashSet<u32> = nodes.iter().map(|n| n.id).collect();
    *guard = Some(RunningEngine { wrapper: engine, _stream: stream, valid_node_ids });
    Ok(())
}

#[tauri::command]
fn stop_engine(state: State<'_, EngineState>) -> Result<(), String> {
    let mut guard = state.0.write().map_err(|e| e.to_string())?;
    if let Some(ref mut eng) = *guard {
        if let Err(e) = eng.wrapper.stop() {
            eprintln!("stop_engine warning: {e}");
        }
    }
    // Always drop the RunningEngine (kills cpal stream + C++ engine)
    *guard = None;
    Ok(())
}

#[tauri::command]
fn set_param(
    node_id: u32,
    param_hash: u32,
    value: f32,
    state: State<'_, EngineState>,
) -> Result<(), String> {
    let guard = state.0.read().map_err(|e| e.to_string())?;
    if let Some(ref eng) = *guard {
        if !eng.valid_node_ids.contains(&node_id) {
            return Err(format!(
                "set_param: node id {node_id} is not in the running graph (param hash 0x{param_hash:x}, value {value})"
            ));
        }
        eng.wrapper.set_param(node_id, param_hash, value)?;
    }
    Ok(())
}

#[tauri::command]
fn get_engine_cpu_load_permil(state: State<'_, EngineState>) -> Result<u32, String> {
    let guard = state.0.read().map_err(|e| e.to_string())?;
    Ok(guard.as_ref().map(|eng| eng.wrapper.cpu_load_permil()).unwrap_or(0))
}

/* -- entry point -------------------------------------------- */

fn main() {
    tauri::Builder::default()
        .manage(EngineState(RwLock::new(None)))
        .invoke_handler(tauri::generate_handler![
            start_engine,
            stop_engine,
            set_param,
            get_engine_cpu_load_permil
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_engine_type_known_strings() {
        assert!(matches!(parse_engine_type("Oscillator"), Ok(NodeType::Oscillator)));
        assert!(matches!(parse_engine_type("Filter"), Ok(NodeType::Filter)));
        assert!(matches!(parse_engine_type("Gain"), Ok(NodeType::Gain)));
        assert!(matches!(parse_engine_type("Output"), Ok(NodeType::Output)));
        assert!(matches!(parse_engine_type("Delay"), Ok(NodeType::Delay)));
        assert!(matches!(parse_engine_type("Effects"), Ok(NodeType::Effects)));
        assert!(matches!(parse_engine_type("Reverb"), Ok(NodeType::Reverb)));
    }

    #[test]
    fn parse_engine_type_rejects_unknown() {
        // Used to silently coerce to NodeType::Gain, which masked frontend
        // bugs as silent wrong-engine-type behaviour at runtime.
        let err = parse_engine_type("NotARealType").expect_err("must not coerce");
        assert!(err.contains("NotARealType"), "unexpected error: {err}");
        assert!(parse_engine_type("").is_err());
        // Case-sensitive — "oscillator" is not "Oscillator"
        assert!(parse_engine_type("oscillator").is_err());
    }

    fn n(id: u32, engine_type: &str) -> EngineNodeInfo {
        EngineNodeInfo {
            id,
            engine_type: engine_type.into(),
            num_inputs: 0,
            num_outputs: 0,
            engine_subtype: 0,
            params: vec![],
        }
    }

    #[test]
    fn resolve_output_picks_unique_output_node() {
        let nodes = vec![n(0, "Oscillator"), n(7, "Output")];
        assert_eq!(resolve_output_node_id(&nodes).unwrap(), 7);
    }

    #[test]
    fn resolve_output_errors_when_missing() {
        let nodes = vec![n(0, "Oscillator"), n(1, "Filter")];
        let err = resolve_output_node_id(&nodes).unwrap_err();
        assert!(err.contains("No Output node"), "unexpected error: {err}");
    }

    #[test]
    fn resolve_output_errors_on_duplicate_outputs() {
        let nodes = vec![n(0, "Output"), n(1, "Output")];
        let err = resolve_output_node_id(&nodes).unwrap_err();
        assert!(err.contains("Multiple Output"), "unexpected error: {err}");
    }
}
