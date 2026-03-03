//! Safe Rust wrapper around the C++ audio engine FFI.
//!
//! Owns the lock-free queues, output ring buffer, and engine lifetime.
//! Automatic cleanup on drop — stops the engine and frees C++ resources.

use crate::ffi::{
    audio_engine_destroy, audio_engine_init, audio_engine_is_running, audio_engine_start,
    audio_engine_stop, AudioEngine, AudioEngineConfig, CompiledGraph, NodeConnection, NodeDesc,
};
use crate::lockfree_queue::{LockFreeRingBuffer, MIDIEventCmd, ParamUpdateCmd, StatusRegister};

use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::Arc;

/// High-level wrapper for the C++ audio engine.
pub struct AudioEngineWrapper {
    engine: *mut AudioEngine,
    // Queues must outlive the engine — Drop order is field-declaration order.
    param_queue: Box<LockFreeRingBuffer<ParamUpdateCmd>>,
    midi_queue: Box<LockFreeRingBuffer<MIDIEventCmd>>,
    #[allow(dead_code)] // kept alive for C++ to read
    status_register: Box<StatusRegister>,
    output_ring: Arc<OutputRingBuffer>,
    sample_rate: u32,
    block_size: u32,
}

/// Lock-free SPSC ring buffer shared between C++ audio thread (writer)
/// and the cpal output callback (reader).  Capacity must be a power of two
/// so that index masking works correctly.
pub struct OutputRingBuffer {
    buffer: Vec<f32>,
    pub head: AtomicUsize,
    pub tail: AtomicUsize,
    mask: usize,
}

impl OutputRingBuffer {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity.is_power_of_two(), "capacity must be power-of-two");
        Self {
            buffer: vec![0.0_f32; capacity],
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            mask: capacity - 1,
        }
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.mask + 1
    }

    /// Pointer to the backing store (for C++ FFI).
    pub fn as_ptr(&self) -> *const f32 {
        self.buffer.as_ptr()
    }

    /// Read up to `dest.len()` samples; returns the count actually read.
    pub fn read(&self, dest: &mut [f32]) -> usize {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Relaxed);
        let available = head.wrapping_sub(tail) & self.mask;
        let n = dest.len().min(available);
        for (i, sample) in dest.iter_mut().enumerate().take(n) {
            *sample = self.buffer[(tail + i) & self.mask];
        }
        if n > 0 {
            self.tail.store((tail + n) & self.mask, Ordering::Release);
        }
        n
    }
}

impl AudioEngineWrapper {
    /// Build and initialise the C++ engine from a compiled graph.
    pub fn new(
        nodes: Vec<NodeDesc>,
        edges: Vec<NodeConnection>,
        execution_order: Vec<u32>,
        output_node_id: u32,
        sample_rate: u32,
        block_size: u32,
        cpu_core: u32,
    ) -> Result<Self, String> {
        let param_queue = LockFreeRingBuffer::<ParamUpdateCmd>::new(8192);
        let midi_queue = LockFreeRingBuffer::<MIDIEventCmd>::new(4096);
        let mut status_register = Box::new(StatusRegister {
            graph_version: AtomicU32::new(0),
            adopted_version: AtomicU32::new(0),
            reserved: [0, 0],
        });

        // Build CompiledGraph from owned vecs.
        // The C++ side copies the data in audio_engine_init, so we reclaim
        // the memory immediately after the call.
        let num_nodes = nodes.len() as u32;
        let num_edges = edges.len() as u32;
        let num_order = execution_order.len() as u32;

        let mut nodes_box = nodes.into_boxed_slice();
        let mut edges_box = edges.into_boxed_slice();
        let mut order_box = execution_order.into_boxed_slice();

        let graph = CompiledGraph {
            nodes: nodes_box.as_mut_ptr() as *const NodeDesc,
            num_nodes,
            connections: edges_box.as_mut_ptr() as *const NodeConnection,
            num_connections: num_edges,
            execution_order: order_box.as_mut_ptr() as *const u32,
            num_in_order: num_order,
            output_node_id,
        };

        let config = AudioEngineConfig {
            sample_rate,
            block_size,
            cpu_core,
        };

        // 64K samples ~ 1.3 s at 48 kHz
        let output_ring = Arc::new(OutputRingBuffer::new(65536));

        // SAFETY: all pointers point into heap allocations that outlive this
        // call. C++ copies graph data and stores ring-buffer pointers.
        // Tail pointers are passed as *mut because C++ (the consumer) writes
        // to them to advance the read position.
        let engine = unsafe {
            audio_engine_init(
                &graph,
                &config,
                param_queue.as_ptr() as *const std::ffi::c_void,
                param_queue.capacity() as u32,
                param_queue.head_ptr() as *const std::ffi::c_void,
                param_queue.tail_ptr() as *mut std::ffi::c_void,
                midi_queue.as_ptr() as *const std::ffi::c_void,
                midi_queue.capacity() as u32,
                midi_queue.head_ptr() as *const std::ffi::c_void,
                midi_queue.tail_ptr() as *mut std::ffi::c_void,
                &mut *status_register,
                output_ring.as_ptr() as *mut f32,
                output_ring.capacity() as u32,
                &output_ring.head as *const AtomicUsize as *mut std::ffi::c_void,
                &output_ring.tail as *const AtomicUsize as *const std::ffi::c_void,
            )
        };

        // graph slices are freed here (drop of nodes_box / edges_box / order_box)
        drop(nodes_box);
        drop(edges_box);
        drop(order_box);

        if engine.is_null() {
            return Err("C++ audio_engine_init returned null".into());
        }

        Ok(Self {
            engine,
            param_queue: Box::new(param_queue),
            midi_queue: Box::new(midi_queue),
            status_register,
            output_ring,
            sample_rate,
            block_size,
        })
    }

    /// Start the real-time audio thread.
    pub fn start(&mut self) -> Result<(), String> {
        match unsafe { audio_engine_start(self.engine) } {
            0 => Ok(()),
            code => Err(format!("audio_engine_start failed (code {code})")),
        }
    }

    /// Stop the audio thread gracefully.
    pub fn stop(&mut self) -> Result<(), String> {
        match unsafe { audio_engine_stop(self.engine) } {
            0 => Ok(()),
            code => Err(format!("audio_engine_stop failed (code {code})")),
        }
    }

    /// Enqueue a parameter change for the audio thread.
    pub fn set_param(&self, node_id: u32, param_hash: u32, value: f32) -> Result<(), String> {
        self.param_queue
            .enqueue(ParamUpdateCmd {
                node_id,
                param_hash,
                value,
                padding: 0,
            })
            .map_err(|_| "param queue full".into())
    }

    /// Enqueue a MIDI event for the audio thread.
    pub fn send_midi_event(
        &self,
        event_type: u32,
        pitch: u32,
        velocity: u32,
        timestamp_samples: u32,
    ) -> Result<(), String> {
        self.midi_queue
            .enqueue(MIDIEventCmd {
                event_type,
                pitch,
                velocity,
                timestamp_samples,
            })
            .map_err(|_| "MIDI queue full".into())
    }

    pub fn is_running(&self) -> bool {
        unsafe { audio_engine_is_running(self.engine) != 0 }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    pub fn block_size(&self) -> u32 {
        self.block_size
    }

    /// Clone the `Arc` to the output ring for the cpal callback.
    pub fn output_ring(&self) -> Arc<OutputRingBuffer> {
        Arc::clone(&self.output_ring)
    }
}

impl Drop for AudioEngineWrapper {
    fn drop(&mut self) {
        if !self.engine.is_null() {
            let _ = self.stop();
            unsafe { audio_engine_destroy(self.engine) };
            self.engine = std::ptr::null_mut();
        }
    }
}

// SAFETY: The wrapper is the sole owner of the engine pointer and all shared
// state is behind atomic operations or lock-free queues.
unsafe impl Send for AudioEngineWrapper {}
unsafe impl Sync for AudioEngineWrapper {}
