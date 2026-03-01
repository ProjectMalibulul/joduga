//! Tauri backend for Joduga.
//!
//! Exposes `start_engine`, `stop_engine`, and `set_param` commands
//! to the React frontend via the Tauri IPC bridge.

use joduga::{
    audio_engine_wrapper::AudioEngineWrapper,
    ffi::NodeType,
    shadow_graph::{Edge, Node, ShadowGraph},
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::State;

/* ── types received from the frontend ──────────────────────── */

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

/* ── managed state ─────────────────────────────────────────── */

struct EngineState(Mutex<Option<AudioEngineWrapper>>);

/* ── helpers ───────────────────────────────────────────────── */

fn parse_engine_type(s: &str) -> NodeType {
    match s {
        "Oscillator" => NodeType::Oscillator,
        "Filter" => NodeType::Filter,
        "Gain" => NodeType::Gain,
        "Output" => NodeType::Output,
        _ => NodeType::Gain, // fallback
    }
}

/* ── commands ──────────────────────────────────────────────── */

#[tauri::command]
fn start_engine(
    nodes: Vec<EngineNodeInfo>,
    edges: Vec<EngineEdgeInfo>,
    state: State<'_, EngineState>,
) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;

    // Stop previous engine if running
    if let Some(ref mut eng) = *guard {
        let _ = eng.stop();
    }
    *guard = None;

    // Find output node
    let output_id = nodes
        .iter()
        .find(|n| n.engine_type == "Output")
        .map(|n| n.id)
        .unwrap_or_else(|| nodes.last().map(|n| n.id).unwrap_or(0));

    // Build shadow graph
    let mut graph = ShadowGraph::new(output_id);

    for n in &nodes {
        graph
            .add_node(Node {
                id: n.id,
                node_type: parse_engine_type(&n.engine_type),
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

    let mut engine = AudioEngineWrapper::new(
        compiled_nodes,
        compiled_edges,
        order,
        output_id,
        48000,
        256,
        0,
    )?;

    // Send initial param values
    for n in &nodes {
        for p in &n.params {
            let _ = engine.set_param(n.id, p.hash, p.value);
        }
    }

    engine.start()?;
    *guard = Some(engine);
    Ok(())
}

#[tauri::command]
fn stop_engine(state: State<'_, EngineState>) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(ref mut eng) = *guard {
        eng.stop()?;
    }
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
        eng.set_param(node_id, param_hash, value)?;
    }
    Ok(())
}

/* ── entry point ───────────────────────────────────────────── */

fn main() {
    tauri::Builder::default()
        .manage(EngineState(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![start_engine, stop_engine, set_param])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
