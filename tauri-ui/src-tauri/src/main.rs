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
use std::sync::{Arc, Mutex};
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

struct EngineState(Mutex<Option<RunningEngine>>);

/// Holds both the C++ engine wrapper and the cpal stream.
/// The stream must be kept alive for audio to play.
struct RunningEngine {
    wrapper: AudioEngineWrapper,
    _stream: cpal::Stream,
}

// cpal::Stream is !Send, but we only touch it behind a Mutex on the main thread
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
fn open_cpal_stream(ring: Arc<OutputRingBuffer>, sample_rate: u32) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or("No audio output device found")?;
    let config = cpal::StreamConfig {
        channels: 1,
        sample_rate: cpal::SampleRate(sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };
    let stream = device
        .build_output_stream(
            &config,
            move |buffer: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let n = ring.read(buffer);
                for sample in &mut buffer[n..] {
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
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;

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

    // Send initial param values
    for n in &nodes {
        for p in &n.params {
            let _ = engine.set_param(n.id, p.hash, p.value);
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
            let _ = engine.set_param(n.id, h, n.engine_subtype as f32);
        }
    }

    // Open cpal output stream BEFORE starting engine
    let ring = engine.output_ring();
    let stream = open_cpal_stream(ring, sample_rate)?;

    engine.start()?;
    *guard = Some(RunningEngine { wrapper: engine, _stream: stream });
    Ok(())
}

#[tauri::command]
fn stop_engine(state: State<'_, EngineState>) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
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
    let guard = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(ref eng) = *guard {
        eng.wrapper.set_param(node_id, param_hash, value)?;
    }
    Ok(())
}

#[tauri::command]
fn get_engine_cpu_load_permil(state: State<'_, EngineState>) -> Result<u32, String> {
    let guard = state.0.lock().map_err(|e| e.to_string())?;
    Ok(guard.as_ref().map(|eng| eng.wrapper.cpu_load_permil()).unwrap_or(0))
}

/* -- entry point -------------------------------------------- */

fn main() {
    tauri::Builder::default()
        .manage(EngineState(Mutex::new(None)))
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
