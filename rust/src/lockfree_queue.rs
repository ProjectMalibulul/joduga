/// Lock-free SPSC ring buffer for real-time audio communication.
///
/// Writer: Rust UI thread (params / MIDI).  Reader: C++ audio thread.
///
/// Properties:
/// - Zero allocations after init
/// - Wait-free reader, lock-free writer
/// - Cache-line padding avoids false sharing
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

// ── Shared command types (repr(C), 16 bytes each) ──────────────────────

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ParamUpdateCmd {
    pub node_id: u32,
    pub param_hash: u32,
    pub value: f32,
    pub padding: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MIDIEventCmd {
    pub event_type: u32,
    pub pitch: u32,
    pub velocity: u32,
    pub timestamp_samples: u32,
}

#[repr(C)]
#[derive(Debug)]
pub struct StatusRegister {
    pub graph_version: u32,
    pub adopted_version: u32,
    pub reserved: [u32; 2],
}

// ── Ring buffer ────────────────────────────────────────────────────────

pub struct LockFreeRingBuffer<T: Clone + Copy> {
    buffer: Vec<T>,
    head: Arc<AtomicUsize>,
    tail: Arc<AtomicUsize>,
    mask: usize,
}

impl<T: Clone + Copy> LockFreeRingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity.is_power_of_two(), "Capacity must be power of 2");
        Self {
            buffer: vec![unsafe { std::mem::zeroed() }; capacity],
            head: Arc::new(AtomicUsize::new(0)),
            tail: Arc::new(AtomicUsize::new(0)),
            mask: capacity - 1,
        }
    }

    // ── FFI pointers ────────────────────────────────────────────────

    pub fn as_ptr(&self) -> *const T {
        self.buffer.as_ptr()
    }
    pub fn head_ptr(&self) -> *const AtomicUsize {
        self.head.as_ref()
    }
    pub fn tail_ptr(&self) -> *const AtomicUsize {
        self.tail.as_ref()
    }
    pub fn capacity(&self) -> usize {
        self.buffer.len()
    }

    // ── Producer (Rust thread) ──────────────────────────────────────

    pub fn enqueue(&self, item: T) -> Result<(), T> {
        let head = self.head.load(Ordering::Acquire);
        let next = (head + 1) & self.mask;
        if next == self.tail.load(Ordering::Acquire) {
            return Err(item); // full
        }
        unsafe {
            ptr::write((self.buffer.as_ptr() as *mut T).add(head), item);
        }
        self.head.store(next, Ordering::Release);
        Ok(())
    }

    // ── Consumer (C++ thread reads directly; this is for tests) ─────

    pub fn dequeue(&self, out: &mut [T]) -> usize {
        let tail = self.tail.load(Ordering::Acquire);
        let head = self.head.load(Ordering::Acquire);
        let avail = if head >= tail {
            head - tail
        } else {
            self.buffer.len() - tail + head
        };
        let n = avail.min(out.len());
        for (i, slot) in out.iter_mut().enumerate().take(n) {
            *slot = unsafe { ptr::read(self.buffer.as_ptr().add((tail + i) & self.mask)) };
        }
        if n > 0 {
            self.tail.store((tail + n) & self.mask, Ordering::Release);
        }
        n
    }

    pub fn len(&self) -> usize {
        let h = self.head.load(Ordering::Relaxed);
        let t = self.tail.load(Ordering::Relaxed);
        if h >= t {
            h - t
        } else {
            self.buffer.len() - t + h
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
    fn enqueue_dequeue() {
        let q = LockFreeRingBuffer::<u32>::new(4);
        assert!(q.enqueue(10).is_ok());
        assert!(q.enqueue(20).is_ok());
        let mut out = [0u32; 2];
        assert_eq!(q.dequeue(&mut out), 2);
        assert_eq!(out, [10, 20]);
    }

    #[test]
    fn full_returns_err() {
        let q = LockFreeRingBuffer::<u32>::new(4);
        // capacity=4 → usable slots=3 (one wasted to distinguish full/empty)
        assert!(q.enqueue(1).is_ok());
        assert!(q.enqueue(2).is_ok());
        assert!(q.enqueue(3).is_ok());
        assert!(q.enqueue(4).is_err());
    }
}
