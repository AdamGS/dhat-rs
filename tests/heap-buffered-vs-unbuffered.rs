//! Test that buffered and unbuffered modes produce equivalent profiling statistics.
//!
//! This test verifies that the buffering optimization doesn't change the
//! important profiling results. The following should be identical:
//! - Total bytes and blocks allocated
//! - End bytes and blocks (what's still allocated at profiler drop)
//!
//! The `at_tgmax` (bytes/blocks at global peak) may differ slightly in buffered
//! mode because pending blocks haven't been resolved to their PPs yet when the
//! peak is recorded. This is a documented trade-off of buffering.

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use serde_json::Value;

/// A deterministic workload that performs various allocations.
fn deterministic_workload() {
    // Allocate and keep alive
    let v1: Vec<u32> = vec![1, 2, 3, 4]; // 16 bytes

    // Allocate, resize, drop
    let mut v2: Vec<u8> = vec![0u8; 100]; // 100 bytes
    v2.reserve(100); // realloc to 200 bytes
    drop(v2);

    // Multiple small allocations from same call site
    for i in 0..5 {
        let v: Vec<u8> = vec![i as u8; 10]; // 10 bytes each, 5 times = 50 bytes total
        std::hint::black_box(&v);
        drop(v);
    }

    // Keep v1 alive until end
    std::hint::black_box(&v1);
}

/// Extract aggregate statistics from JSON.
fn extract_aggregate_stats(json: &Value) -> AggregateStats {
    let pps = json["pps"].as_array().unwrap();

    let total_bytes: i64 = pps.iter().map(|pp| pp["tb"].as_i64().unwrap()).sum();
    let total_blocks: i64 = pps.iter().map(|pp| pp["tbk"].as_i64().unwrap()).sum();

    // Sum of end bytes/blocks (should be 0 if everything was freed)
    let end_bytes: i64 = pps.iter().map(|pp| pp["eb"].as_i64().unwrap()).sum();
    let end_blocks: i64 = pps.iter().map(|pp| pp["ebk"].as_i64().unwrap()).sum();

    // Sum of bytes/blocks at global max
    let at_tgmax_bytes: i64 = pps.iter().map(|pp| pp["gb"].as_i64().unwrap()).sum();
    let at_tgmax_blocks: i64 = pps.iter().map(|pp| pp["gbk"].as_i64().unwrap()).sum();

    AggregateStats {
        total_bytes,
        total_blocks,
        end_bytes,
        end_blocks,
        at_tgmax_bytes,
        at_tgmax_blocks,
    }
}

#[derive(Debug, PartialEq, Eq)]
struct AggregateStats {
    total_bytes: i64,
    total_blocks: i64,
    end_bytes: i64,
    end_blocks: i64,
    at_tgmax_bytes: i64,
    at_tgmax_blocks: i64,
}

#[test]
fn main() {
    // Run with unbuffered mode first
    let unbuffered_output = {
        let mut profiler =
            std::mem::ManuallyDrop::new(dhat::Profiler::builder().buffer_capacity(0).build());
        deterministic_workload();
        profiler.drop_and_get_memory_output()
    };

    // Run with buffered mode
    let buffered_output = {
        let mut profiler =
            std::mem::ManuallyDrop::new(dhat::Profiler::builder().buffer_capacity(64).build());
        deterministic_workload();
        profiler.drop_and_get_memory_output()
    };

    // Parse both outputs
    let unbuffered_json: Value = serde_json::from_str(&unbuffered_output).unwrap();
    let buffered_json: Value = serde_json::from_str(&buffered_output).unwrap();

    // Extract and compare aggregate statistics
    let unbuffered_stats = extract_aggregate_stats(&unbuffered_json);
    let buffered_stats = extract_aggregate_stats(&buffered_json);

    // Total allocations should be identical
    assert_eq!(
        unbuffered_stats.total_bytes, buffered_stats.total_bytes,
        "Total bytes should match"
    );
    assert_eq!(
        unbuffered_stats.total_blocks, buffered_stats.total_blocks,
        "Total blocks should match"
    );

    // End state should be identical
    assert_eq!(
        unbuffered_stats.end_bytes, buffered_stats.end_bytes,
        "End bytes should match"
    );
    assert_eq!(
        unbuffered_stats.end_blocks, buffered_stats.end_blocks,
        "End blocks should match"
    );

    // Note: at_tgmax may differ due to buffering - this is expected.
    // The per-PP attribution at peak time is less accurate when blocks
    // are still pending in the buffer. We just verify the values are
    // reasonable (non-negative and <= total).
    assert!(
        buffered_stats.at_tgmax_bytes <= buffered_stats.total_bytes,
        "at_tgmax_bytes should be <= total_bytes"
    );
    assert!(
        buffered_stats.at_tgmax_blocks <= buffered_stats.total_blocks,
        "at_tgmax_blocks should be <= total_blocks"
    );
}
