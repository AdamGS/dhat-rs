//! Test that thread buffers are properly flushed when profiler drops,
//! even if threads are still running (simulating thread pool behavior).

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;

#[test]
fn main() {
    let keep_running = Arc::new(AtomicBool::new(true));
    let started = Arc::new(Barrier::new(3)); // 2 worker threads + main

    let mem = {
        let mut profiler =
            std::mem::ManuallyDrop::new(dhat::Profiler::builder().buffer_capacity(64).build());

        // Spawn "worker" threads that stay alive like a thread pool
        let handles: Vec<_> = (0..2)
            .map(|i| {
                let keep_running = Arc::clone(&keep_running);
                let started = Arc::clone(&started);
                thread::spawn(move || {
                    // Allocate something
                    let v: Vec<u32> = vec![i as u32; 50];
                    assert_eq!(v.len(), 50);

                    // Signal that we've allocated
                    started.wait();

                    // Simulate thread pool - keep thread alive
                    while keep_running.load(Ordering::Relaxed) {
                        thread::yield_now();
                    }
                })
            })
            .collect();

        // Wait for workers to allocate
        started.wait();

        // Drop profiler while threads are still running
        let output = profiler.drop_and_get_memory_output();

        // Now signal threads to exit
        keep_running.store(false, Ordering::Relaxed);
        for handle in handles {
            handle.join().unwrap();
        }

        output
    };

    // Parse the output and verify allocations were tracked
    let v: serde_json::Value = serde_json::from_str(&mem).unwrap();

    // Check that we have program points recorded
    let pps = v["pps"].as_array().unwrap();
    assert!(!pps.is_empty(), "Should have recorded some allocations");

    // Total bytes should include the worker thread allocations
    // Each thread allocated vec![i; 50] where i is 0 or 1, so 50 u32s = 200 bytes each
    let total_bytes: i64 = pps.iter().map(|pp| pp["tb"].as_i64().unwrap()).sum();
    assert!(
        total_bytes >= 400,
        "Expected at least 400 bytes from worker threads, got {}",
        total_bytes
    );
}
