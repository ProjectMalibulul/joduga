use std::ptr;
/// Lock-free SPSC (Single Producer, Single Consumer) ring buffer for real-time safety.
///
/// This is the core synchronization mechanism between the Rust UI thread and the
/// C++ audio thread. The Rust thread writes commands (parameter updates, MIDI events),
/// and the C++ audio thread reads and drains them.
///
/// Key properties:
/// - Zero allocations after initialization
/// - No mutexes (wait-free reader, lock-free writer)
/// - Cache-line aligned to avoid false sharing
/// - Typical usage: 8KB ring buffer for ~500 parameter updates per block
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Parameter update command (16 bytes, aligned to cache line)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ParamUpdateCmd {
    pub node_id: u32,
    pub param_hash: u32,
    pub value: f32,
    pub padding: u32,
}

/// MIDI event command (16 bytes)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MIDIEventCmd {
    pub event_type: u32,
    pub pitch: u32,
    pub velocity: u32,
    pub timestamp_samples: u32,
}

/// Status register shared between Rust and C++
#[repr(C)]
#[derive(Debug)]
pub struct StatusRegister {
    pub graph_version: u32,
    pub adopted_version: u32,
    pub reserved: [u32; 2],
}

/// Lock-free ring buffer for SPSC communication
pub struct LockFreeRingBuffer<T: Clone + Copy> {
    buffer: Vec<T>,
    head: Arc<AtomicUsize>, // Write pointer (Rust thread)
    tail: Arc<AtomicUsize>, // Read pointer (C++ thread)
    mask: usize,
}

impl<T: Clone + Copy> LockFreeRingBuffer<T> {
    /// Create a new ring buffer with capacity (must be power of 2)
    pub fn new(capacity: usize) -> Self {
        assert!(capacity.is_power_of_two(), "Capacity must be a power of 2");

        let buffer = vec![unsafe { std::mem::zeroed() }; capacity];
        LockFreeRingBuffer {
            buffer,
            head: Arc::new(AtomicUsize::new(0)),
            tail: Arc::new(AtomicUsize::new(0)),
            mask: capacity - 1,
        }
    }

    /// Get a pointer to the raw buffer (for FFI)
    pub fn as_ptr(&self) -> *const T {
        self.buffer.as_ptr()
    }

    /// Get mutable pointer (for C++ to write into)
    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.buffer.as_mut_ptr()
    }

    /// Enqueue an item (Rust side, can block if full)
    pub fn enqueue(&self, item: T) -> Result<(), T> {
        let head = self.head.load(Ordering::Acquire);
        let next_head = (head + 1) & self.mask;
        let tail = self.tail.load(Ordering::Acquire);

        if next_head == tail {
            // Buffer is full
            return Err(item);
        }

        unsafe {
            ptr::write((self.buffer.as_ptr() as *mut T).add(head), item);
        }

        self.head.store(next_head, Ordering::Release);
        Ok(())
    }

    /// Enqueue multiple items (Rust side)
    pub fn enqueue_slice(&self, items: &[T]) -> usize {
        let mut written = 0;
        for item in items {
            if self.enqueue(*item).is_ok() {
                written += 1;
            } else {
                break;
            }
        }
        written
    }

    /// Dequeue items into a buffer (C++ side, called from audio thread)
    /// Returns the number of items dequeued
    pub fn dequeue(&self, out: &mut [T]) -> usize {
        let tail = self.tail.load(Ordering::Acquire);
        let head = self.head.load(Ordering::Acquire);

        let available = if head >= tail {
            head - tail
        } else {
            self.buffer.len() - tail + head
        };

        let to_read = std::cmp::min(available, out.len());

        for i in 0..to_read {
            out[i] = unsafe {
                ptr::read((self.buffer.as_ptr() as *const T).add((tail + i) & self.mask))
            };
        }

        if to_read > 0 {
            self.tail
                .store((tail + to_read) & self.mask, Ordering::Release);
        }

        to_read
    }

    /// Get pointer to head index (for C++ to read)
    pub fn head_ptr(&self) -> *const AtomicUsize {
        self.head.as_ref()
    }

    /// Get pointer to tail index (for C++ to write)
    pub fn tail_ptr(&self) -> *const AtomicUsize {
        self.tail.as_ref()
    }

    /// Get capacity
    pub fn capacity(&self) -> usize {
        self.buffer.len()
    }

    /// Get approximate number of items in the buffer
    pub fn len(&self) -> usize {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Relaxed);
        if head >= tail {
            head - tail
        } else {
            self.buffer.len() - tail + head
        }
    }

    pub fn is_empty(&self) -> bool {
        self.head.load(Ordering::Relaxed) == self.tail.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enqueue_dequeue() {
        let queue = LockFreeRingBuffer::<u32>::new(4);

        assert!(queue.enqueue(10).is_ok());
        assert!(queue.enqueue(20).is_ok());

        let mut out = [0u32; 2];
        let n = queue.dequeue(&mut out);
        assert_eq!(n, 2);
        assert_eq!(out[0], 10);
        assert_eq!(out[1], 20);
    }

    #[test]
    fn test_wrap_around() {
        let queue = LockFreeRingBuffer::<u32>::new(4);

        for i in 0..8 {
            queue.enqueue(i).unwrap();
        }

        let mut out = [0u32; 4];
        queue.dequeue(&mut out);
        assert_eq!(out[0], 4);
    }
}
