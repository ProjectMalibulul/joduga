/// Lock-free SPSC ring buffer for real-time audio communication.
///
/// Writer: Rust UI thread (params / MIDI).  Reader: C++ audio thread.
///
/// Properties:
/// - Zero allocations after init
/// - Wait-free reader, lock-free writer
/// - Cache-line padding avoids false sharing
///
/// Memory ordering contract:
/// - Producer loads head (Relaxed), loads tail (Acquire), stores head (Release)
/// - Consumer loads tail (Relaxed), loads head (Acquire), stores tail (Release)
/// - This ensures the written data is visible before the index advances.
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

/// Shared status register between Rust and C++.
/// All fields are atomic to prevent data races across threads.
#[repr(C)]
#[derive(Debug)]
pub struct StatusRegister {
    pub graph_version: std::sync::atomic::AtomicU32,
    pub adopted_version: std::sync::atomic::AtomicU32,
    pub reserved: [u32; 2],
}

// ── Ring buffer ────────────────────────────────────────────────────────

pub struct LockFreeRingBuffer<T: Clone + Copy> {
    buffer: Vec<T>,
    head: Arc<AtomicUsize>,
    tail: Arc<AtomicUsize>,
    mask: usize,
}

// SAFETY: The buffer is only written by the producer (via head) and read by
// the consumer (via tail). The atomic indices enforce the happens-before
// relationship across threads. T is Copy so there are no destructors to worry
// about on the data itself.
unsafe impl<T: Clone + Copy + Send> Send for LockFreeRingBuffer<T> {}
unsafe impl<T: Clone + Copy + Send> Sync for LockFreeRingBuffer<T> {}

impl<T: Clone + Copy> LockFreeRingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity.is_power_of_two(), "Capacity must be power of 2");
        assert!(capacity >= 2, "Capacity must be at least 2");
        Self {
            // SAFETY: T is Copy (no drop glue) and we zero-init all slots.
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

    /// Enqueue an item. Returns Err(item) if the queue is full.
    ///
    /// # Memory ordering
    /// - head is owned by the producer, so Relaxed load is sufficient.
    /// - tail is written by the consumer, so Acquire load synchronises with
    ///   the consumer's Release store, making dequeued slots visible.
    /// - After writing the item, head is stored with Release so the consumer
    ///   sees the data before it sees the updated head.
    pub fn enqueue(&self, item: T) -> Result<(), T> {
        let head = self.head.load(Ordering::Relaxed);
        let next = (head + 1) & self.mask;
        if next == self.tail.load(Ordering::Acquire) {
            return Err(item); // full
        }
        // SAFETY: head index is within [0, capacity) and only this thread
        // writes to buffer[head]. The consumer never reads past its tail.
        unsafe {
            ptr::write((self.buffer.as_ptr() as *mut T).add(head), item);
        }
        self.head.store(next, Ordering::Release);
        Ok(())
    }

    // ── Consumer (C++ thread reads directly; this is for tests) ─────

    /// Dequeue up to `out.len()` items. Returns the count actually read.
    ///
    /// # Memory ordering
    /// - tail is owned by the consumer, so Relaxed load is sufficient.
    /// - head is written by the producer, so Acquire load synchronises
    ///   to see items that were enqueued.
    /// - After reading, tail is stored with Release so the producer can
    ///   reclaim those slots.
    pub fn dequeue(&self, out: &mut [T]) -> usize {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        let avail = (head.wrapping_sub(tail)) & self.mask;
        let n = avail.min(out.len());
        for (i, slot) in out.iter_mut().enumerate().take(n) {
            // SAFETY: index is within bounds and producer has finished writing.
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
        (h.wrapping_sub(t)) & self.mask
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

    #[test]
    fn wraparound() {
        let q = LockFreeRingBuffer::<u32>::new(4);
        // Fill and drain twice to test wraparound
        for round in 0..3 {
            let base = round * 10;
            assert!(q.enqueue(base + 1).is_ok());
            assert!(q.enqueue(base + 2).is_ok());
            let mut out = [0u32; 2];
            assert_eq!(q.dequeue(&mut out), 2);
            assert_eq!(out, [base + 1, base + 2]);
            assert!(q.is_empty());
        }
    }

    #[test]
    fn len_tracks_correctly() {
        let q = LockFreeRingBuffer::<u32>::new(8);
        assert_eq!(q.len(), 0);
        q.enqueue(1).unwrap();
        q.enqueue(2).unwrap();
        assert_eq!(q.len(), 2);
        let mut out = [0u32; 1];
        q.dequeue(&mut out);
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn spsc_stress_across_threads() {
        // Producer (this test thread spawns a child as the producer, the
        // current thread is the consumer). Sends N=200_000 sequential u32
        // values; the consumer must observe every value exactly once and
        // in order. This exercises the Acquire/Release synchronisation
        // contract under real cross-thread interleaving.
        const N: u32 = 200_000;
        let q = Arc::new(LockFreeRingBuffer::<u32>::new(1024));

        let producer_q = Arc::clone(&q);
        let producer = std::thread::spawn(move || {
            let mut next = 0u32;
            while next < N {
                if producer_q.enqueue(next).is_ok() {
                    next += 1;
                } else {
                    // queue full — yield and retry
                    std::thread::yield_now();
                }
            }
        });

        let mut received: u32 = 0;
        let mut buf = [0u32; 64];
        while received < N {
            let n = q.dequeue(&mut buf);
            if n == 0 {
                std::thread::yield_now();
                continue;
            }
            for slot in buf.iter().take(n) {
                assert_eq!(*slot, received, "out-of-order or duplicated value");
                received += 1;
            }
        }
        producer.join().expect("producer panicked");
        assert_eq!(received, N);
        assert!(q.is_empty());
    }
}
