/// Minimal benchmark: measures lock-free queue throughput.
///
/// Run with:  cargo run --example bench_queue -p joduga --release
use joduga::lockfree_queue::{LockFreeRingBuffer, ParamUpdateCmd};
use std::time::Instant;

const CAPACITY: usize = 8192;
const ITERATIONS: usize = 1_000_000;

fn main() {
    let queue: LockFreeRingBuffer<ParamUpdateCmd> = LockFreeRingBuffer::new(CAPACITY);

    let item = ParamUpdateCmd { node_id: 1, param_hash: 0, value: 0.5, padding: 0 };

    let mut buf = [item; 1];

    // ── Single-thread enqueue/dequeue throughput ────────────────────
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _ = queue.enqueue(item);
        queue.dequeue(&mut buf);
    }
    let elapsed = start.elapsed();

    let ops = ITERATIONS as f64 * 2.0; // enqueue + dequeue
    let ns_per_op = elapsed.as_nanos() as f64 / ops;
    let mops = ops / elapsed.as_secs_f64() / 1_000_000.0;

    println!("Joduga v{}", joduga::VERSION);
    println!("Lock-free queue benchmark (single-thread, capacity={CAPACITY})");
    println!("  {ITERATIONS} enqueue+dequeue pairs");
    println!("  Total: {elapsed:?}");
    println!("  {ns_per_op:.1} ns/op  ({mops:.2} Mops/s)");

    // ── Burst fill + drain ──────────────────────────────────────────
    let burst = CAPACITY - 1; // max fillable
    let mut drain_buf = vec![item; burst];
    let start = Instant::now();
    for _ in 0..1000 {
        for _ in 0..burst {
            let _ = queue.enqueue(item);
        }
        queue.dequeue(&mut drain_buf);
    }
    let elapsed = start.elapsed();
    let total_ops = burst as f64 * 2.0 * 1000.0;
    let ns_per = elapsed.as_nanos() as f64 / total_ops;
    println!("\nBurst fill/drain ({burst} items × 1000 rounds)");
    println!("  Total: {elapsed:?}");
    println!("  {ns_per:.1} ns/op");
}
