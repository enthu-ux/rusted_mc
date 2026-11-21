//! Demonstrates the allocation-counting global allocator.
//!
//! `cargo run -p mc-perf --example alloc_probe` prints allocations/bytes/frees for a small workload.

use std::alloc::System;

static COUNTERS: mc_perf::alloc::Counters = mc_perf::alloc::Counters::new();

#[global_allocator]
static ALLOC: mc_perf::alloc::CountingAllocator<System> =
    mc_perf::alloc::CountingAllocator::new(System, &COUNTERS);

fn main() {
    COUNTERS.set_enabled(true);
    COUNTERS.reset();

    let mut v: Vec<u64> = Vec::new();
    for i in 0..1000u64 {
        v.push(i);
    }
    let s = COUNTERS.snapshot();
    println!(
        "{{\"allocations\":{},\"allocated_bytes\":{},\"frees\":{},\"freed_bytes\":{},\"net_bytes\":{}}}",
        s.allocations,
        s.allocated_bytes,
        s.frees,
        s.freed_bytes,
        s.net_bytes()
    );
    drop(v);
}
