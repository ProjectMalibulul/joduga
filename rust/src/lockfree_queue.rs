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

#[repr(C, align(16))]
#[derive(Debug, Clone, Copy)]
pub struct ParamUpdateCmd {
    pub node_id: u32,
    pub param_hash: u32,
    pub value: f32,
    pub padding: u32,
}

#[repr(C, align(16))]
#[derive(Debug, Clone, Copy)]
pub struct MIDIEventCmd {
    pub event_type: u32,
    pub pitch: u32,
    pub velocity: u32,
    pub timestamp_samples: u32,
}

/// Shared status register between Rust and C++.
/// Plain integers are used to preserve ABI layout with C++.
/// Access atomically via AtomicU32::from_ptr on the accessor side.
#[repr(C)]
#[derive(Debug)]
pub struct StatusRegister {
    pub graph_version: u32,
    pub adopted_version: u32,
    pub cpu_load_permil: u32,
    pub reserved: u32,
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
    fn cmd_structs_match_cpp_alignas_contract() {
        // C++ side declares both with alignas(16); Rust must match or the FFI
        // ABI is silently violated. Sizes stay 16 because both structs are
        // already 4 × u32 with no trailing padding.
        assert_eq!(std::mem::align_of::<ParamUpdateCmd>(), 16);
        assert_eq!(std::mem::align_of::<MIDIEventCmd>(), 16);
        assert_eq!(std::mem::size_of::<ParamUpdateCmd>(), 16);
        assert_eq!(std::mem::size_of::<MIDIEventCmd>(), 16);

        // Vec<T> allocates at align_of::<T>(), so the queue's backing
        // storage must now be 16-byte aligned at its base.
        let pq = LockFreeRingBuffer::<ParamUpdateCmd>::new(8);
        let mq = LockFreeRingBuffer::<MIDIEventCmd>::new(8);
        assert_eq!(pq.as_ptr() as usize % 16, 0);
        assert_eq!(mq.as_ptr() as usize % 16, 0);
    }

    /// Pin field layout to cpp/include/audio_engine.h:71-78. A reorder
    /// would silently route param updates to the wrong dispatch field
    /// (e.g. param_hash arriving in the slot that the C++ side reads as
    /// node_id).
    #[test]
    fn param_update_cmd_field_offsets_match_cpp() {
        use std::mem::offset_of;
        assert_eq!(offset_of!(ParamUpdateCmd, node_id), 0);
        assert_eq!(offset_of!(ParamUpdateCmd, param_hash), 4);
        assert_eq!(offset_of!(ParamUpdateCmd, value), 8);
        assert_eq!(offset_of!(ParamUpdateCmd, padding), 12);
    }

    /// MIDIEventCmd has no public C++ counterpart in audio_engine.h yet
    /// but is read by the audio thread off the same queue layout.
    /// Pinning offsets prevents silent drift when one is added.
    #[test]
    fn midi_event_cmd_field_offsets() {
        use std::mem::offset_of;
        assert_eq!(offset_of!(MIDIEventCmd, event_type), 0);
        assert_eq!(offset_of!(MIDIEventCmd, pitch), 4);
        assert_eq!(offset_of!(MIDIEventCmd, velocity), 8);
        assert_eq!(offset_of!(MIDIEventCmd, timestamp_samples), 12);
    }

    /// Pin StatusRegister layout to cpp/include/audio_engine.h:83-89.
    /// Cross-language atomic_ref / AtomicU32::from_ptr access on
    /// cpu_load_permil depends on its offset being exactly 8.
    #[test]
    fn status_register_field_offsets_match_cpp() {
        use std::mem::{align_of, offset_of, size_of};
        assert_eq!(size_of::<StatusRegister>(), 16);
        assert_eq!(align_of::<StatusRegister>(), 4);
        assert_eq!(offset_of!(StatusRegister, graph_version), 0);
        assert_eq!(offset_of!(StatusRegister, adopted_version), 4);
        assert_eq!(offset_of!(StatusRegister, cpu_load_permil), 8);
        assert_eq!(offset_of!(StatusRegister, reserved), 12);
    }
}
