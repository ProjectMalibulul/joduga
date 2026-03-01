use crate::ffi::{
    audio_engine_destroy, audio_engine_init, audio_engine_is_running, audio_engine_start,
    audio_engine_stop, AudioEngine, AudioEngineConfig, CompiledGraph, NodeConnection, NodeDesc,
    NodeType,
};
use crate::lockfree_queue::{LockFreeRingBuffer, MIDIEventCmd, ParamUpdateCmd, StatusRegister};
use std::sync::atomic::AtomicUsize;
/// Safe Rust wrapper around the C++ audio engine FFI.
///
/// This provides:
/// - Automatic initialization and cleanup
/// - Safe lock-free queue management
/// - A high-level API for starting/stopping the engine
/// - Proper error handling
use std::sync::Arc;

/// High-level wrapper for the audio engine
pub struct AudioEngineWrapper {
    engine: *mut AudioEngine,

    // Keep the queues alive as long as the engine is running
    param_queue: Box<LockFreeRingBuffer<ParamUpdateCmd>>,
    midi_queue: Box<LockFreeRingBuffer<MIDIEventCmd>>,
    status_register: Box<StatusRegister>,

    sample_rate: u32,
    block_size: u32,
}

impl AudioEngineWrapper {
    /// Initialize a new audio engine with the given graph.
    ///
    /// # Arguments:
    /// - nodes: Vector of NodeDesc structures
    /// - edges: Vector of connections between nodes
    /// - execution_order: Topologically sorted node indices
    /// - output_node_id: ID of the final output node
    /// - sample_rate: Sample rate in Hz (typically 48000)
    /// - block_size: Number of samples to process per block (typically 256-512)
    /// - cpu_core: CPU core to pin the audio thread to
    ///
    /// # Returns:
    /// Ok(AudioEngineWrapper) on success, Err(String) on failure
    pub fn new(
        nodes: Vec<NodeDesc>,
        edges: Vec<NodeConnection>,
        execution_order: Vec<u32>,
        output_node_id: u32,
        sample_rate: u32,
        block_size: u32,
        cpu_core: u32,
    ) -> Result<Self, String> {
        // Create lock-free queues
        let param_queue = LockFreeRingBuffer::<ParamUpdateCmd>::new(8192); // 8KB
        let midi_queue = LockFreeRingBuffer::<MIDIEventCmd>::new(4096); // 4KB
        let mut status_register = Box::new(StatusRegister {
            graph_version: 0,
            adopted_version: 0,
            reserved: [0, 0],
        });

        // Allocate storage for graph structures
        let num_nodes = nodes.len() as u32;
        let num_edges = edges.len() as u32;
        let num_order = execution_order.len() as u32;

        let nodes_ptr = Box::into_raw(nodes.into_boxed_slice()) as *const NodeDesc;
        let edges_ptr = Box::into_raw(edges.into_boxed_slice()) as *const NodeConnection;
        let order_ptr = Box::into_raw(execution_order.into_boxed_slice()) as *const u32;

        // Create CompiledGraph
        let graph = CompiledGraph {
            nodes: nodes_ptr,
            num_nodes,
            connections: edges_ptr,
            num_connections: num_edges,
            execution_order: order_ptr,
            num_in_order: num_order,
            output_node_id,
        };

        let config = AudioEngineConfig {
            sample_rate,
            block_size,
            cpu_core,
        };

        // Get raw pointers for lock-free structures
        let param_q_ptr = param_queue.as_ptr();
        let param_q_cap = param_queue.capacity() as u32;
        let param_head_ptr = param_queue.head_ptr();
        let param_tail_ptr = param_queue.tail_ptr();

        let midi_q_ptr = midi_queue.as_ptr();
        let midi_q_cap = midi_queue.capacity() as u32;
        let midi_head_ptr = midi_queue.head_ptr();
        let midi_tail_ptr = midi_queue.tail_ptr();

        // Unsafe FFI call
        let engine = unsafe {
            audio_engine_init(
                &graph,
                &config,
                param_q_ptr as *const std::ffi::c_void,
                param_q_cap,
                param_head_ptr as *const std::ffi::c_void,
                param_tail_ptr as *const std::ffi::c_void,
                midi_q_ptr as *const std::ffi::c_void,
                midi_q_cap,
                midi_head_ptr as *const std::ffi::c_void,
                midi_tail_ptr as *const std::ffi::c_void,
                &mut *status_register,
            )
        };

        if engine.is_null() {
            return Err("Failed to initialize audio engine".to_string());
        }

        Ok(AudioEngineWrapper {
            engine,
            param_queue: Box::new(param_queue),
            midi_queue: Box::new(midi_queue),
            status_register,
            sample_rate,
            block_size,
        })
    }

    /// Start the audio engine (spawns the real-time audio thread)
    pub fn start(&mut self) -> Result<(), String> {
        let ret = unsafe { audio_engine_start(self.engine) };
        if ret != 0 {
            Err("Failed to start audio engine".to_string())
        } else {
            Ok(())
        }
    }

    /// Stop the audio engine gracefully
    pub fn stop(&mut self) -> Result<(), String> {
        let ret = unsafe { audio_engine_stop(self.engine) };
        if ret != 0 {
            Err("Failed to stop audio engine".to_string())
        } else {
            Ok(())
        }
    }

    /// Send a parameter update to a node
    pub fn set_param(&self, node_id: u32, param_hash: u32, value: f32) -> Result<(), String> {
        let cmd = ParamUpdateCmd {
            node_id,
            param_hash,
            value,
            padding: 0,
        };

        self.param_queue
            .enqueue(cmd)
            .map_err(|_| "Parameter queue full".to_string())
    }

    /// Send a MIDI event
    pub fn send_midi_event(
        &self,
        event_type: u32,
        pitch: u32,
        velocity: u32,
        timestamp_samples: u32,
    ) -> Result<(), String> {
        let cmd = MIDIEventCmd {
            event_type,
            pitch,
            velocity,
            timestamp_samples,
        };

        self.midi_queue
            .enqueue(cmd)
            .map_err(|_| "MIDI queue full".to_string())
    }

    /// Check if the audio engine is running
    pub fn is_running(&self) -> bool {
        unsafe { audio_engine_is_running(self.engine) != 0 }
    }

    /// Get sample rate
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get block size
    pub fn block_size(&self) -> u32 {
        self.block_size
    }
}

impl Drop for AudioEngineWrapper {
    fn drop(&mut self) {
        if !self.engine.is_null() {
            // Stop if running
            let _ = self.stop();

            // Destroy
            unsafe { audio_engine_destroy(self.engine) };
            self.engine = std::ptr::null_mut();
        }
    }
}

// Unsafe is OK here because we manage lifetime via Drop
unsafe impl Send for AudioEngineWrapper {}
unsafe impl Sync for AudioEngineWrapper {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_param_queue() {
        let queue = Arc::new(LockFreeRingBuffer::<ParamUpdateCmd>::new(256));

        let cmd = ParamUpdateCmd {
            node_id: 1,
            param_hash: 42,
            value: 3.14,
            padding: 0,
        };

        assert!(queue.enqueue(cmd).is_ok());

        let mut out = [ParamUpdateCmd {
            node_id: 0,
            param_hash: 0,
            value: 0.0,
            padding: 0,
        }; 1];

        let n = queue.dequeue(&mut out);
        assert_eq!(n, 1);
        assert_eq!(out[0].node_id, 1);
    }
}
