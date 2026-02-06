//! Benchmark comparing buffered vs unbuffered allocation tracking.
//!
//! Run with:
//!   cargo run --release --example buffer_benchmark 2>/dev/null
//!
//! This benchmark measures the overhead of allocation tracking in multi-threaded
//! scenarios, comparing:
//! - No profiling (baseline)
//! - Unbuffered profiling (buffer_capacity = 0)
//! - Buffered profiling (buffer_capacity = 64, the default)

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

const NUM_THREADS: usize = 16;
const ALLOCS_PER_THREAD: usize = 5_000;
const WARMUP_ITERATIONS: usize = 2;
const BENCH_ITERATIONS: usize = 5;

fn run_workload(barrier: Arc<Barrier>) {
    // Wait for all threads to be ready
    barrier.wait();

    for i in 0..ALLOCS_PER_THREAD {
        // Allocate varying sizes to simulate realistic workload
        let size = 16 + (i % 256);
        let v: Vec<u8> = vec![0u8; size];
        // Prevent optimization
        std::hint::black_box(&v);
        drop(v);
    }

    // Wait for all threads to finish
    barrier.wait();
}

fn benchmark_no_profiling() -> Duration {
    let barrier = Arc::new(Barrier::new(NUM_THREADS + 1));

    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || run_workload(barrier))
        })
        .collect();

    // Start timing
    barrier.wait();
    let start = Instant::now();

    // Wait for completion
    barrier.wait();
    let elapsed = start.elapsed();

    for handle in handles {
        handle.join().unwrap();
    }

    elapsed
}

fn benchmark_unbuffered() -> Duration {
    let _profiler = dhat::Profiler::builder()
        .buffer_capacity(0) // Disable buffering
        .file_name("/dev/null")
        .build();

    let barrier = Arc::new(Barrier::new(NUM_THREADS + 1));

    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || run_workload(barrier))
        })
        .collect();

    // Start timing
    barrier.wait();
    let start = Instant::now();

    // Wait for completion
    barrier.wait();
    let elapsed = start.elapsed();

    for handle in handles {
        handle.join().unwrap();
    }

    elapsed
}

fn benchmark_buffered(buffer_capacity: usize) -> Duration {
    let _profiler = dhat::Profiler::builder()
        .buffer_capacity(buffer_capacity)
        .file_name("/dev/null")
        .build();

    let barrier = Arc::new(Barrier::new(NUM_THREADS + 1));

    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || run_workload(barrier))
        })
        .collect();

    // Start timing
    barrier.wait();
    let start = Instant::now();

    // Wait for completion
    barrier.wait();
    let elapsed = start.elapsed();

    for handle in handles {
        handle.join().unwrap();
    }

    elapsed
}

fn run_benchmark<F>(name: &str, warmup: usize, iterations: usize, mut f: F) -> Duration
where
    F: FnMut() -> Duration,
{
    // Warmup
    for _ in 0..warmup {
        f();
    }

    // Actual benchmark
    let mut total = Duration::ZERO;
    let mut times = Vec::with_capacity(iterations);

    for _ in 0..iterations {
        let elapsed = f();
        times.push(elapsed);
        total += elapsed;
    }

    let avg = total / iterations as u32;
    let min = *times.iter().min().unwrap();
    let max = *times.iter().max().unwrap();

    println!(
        "{:25} avg: {:>10.2?}  min: {:>10.2?}  max: {:>10.2?}",
        name, avg, min, max
    );

    avg
}

fn print_comparison(name: &str, test: Duration, baseline: Duration, unbuffered: Duration) {
    let vs_baseline = test.as_secs_f64() / baseline.as_secs_f64();
    let vs_unbuffered = if unbuffered > test {
        format!(
            "{:>5.1}% faster",
            (1.0 - test.as_secs_f64() / unbuffered.as_secs_f64()) * 100.0
        )
    } else {
        format!(
            "{:>5.1}% slower",
            (test.as_secs_f64() / unbuffered.as_secs_f64() - 1.0) * 100.0
        )
    };
    println!(
        "  {:20} {:>6.1}x baseline   {}",
        name, vs_baseline, vs_unbuffered
    );
}

fn main() {
    println!("Buffer Benchmark");
    println!("================");
    println!();
    println!("Configuration:");
    println!("  Threads:           {}", NUM_THREADS);
    println!("  Allocs per thread: {}", ALLOCS_PER_THREAD);
    println!("  Total allocations: {}", NUM_THREADS * ALLOCS_PER_THREAD);
    println!("  Warmup iterations: {}", WARMUP_ITERATIONS);
    println!("  Bench iterations:  {}", BENCH_ITERATIONS);
    println!();
    println!("NOTE: Buffered mode captures backtrace outside the lock, reducing");
    println!("      lock hold time. Benefits are most visible with high contention.");
    println!();
    println!("Results (avg time):");
    println!();

    let baseline = run_benchmark(
        "No profiling (baseline)",
        WARMUP_ITERATIONS,
        BENCH_ITERATIONS,
        benchmark_no_profiling,
    );

    let unbuffered = run_benchmark(
        "Unbuffered (capacity=0)",
        WARMUP_ITERATIONS,
        BENCH_ITERATIONS,
        benchmark_unbuffered,
    );

    let buffered_64 = run_benchmark(
        "Buffered (capacity=64)",
        WARMUP_ITERATIONS,
        BENCH_ITERATIONS,
        || benchmark_buffered(64),
    );

    let buffered_128 = run_benchmark(
        "Buffered (capacity=128)",
        WARMUP_ITERATIONS,
        BENCH_ITERATIONS,
        || benchmark_buffered(128),
    );

    let buffered_256 = run_benchmark(
        "Buffered (capacity=256)",
        WARMUP_ITERATIONS,
        BENCH_ITERATIONS,
        || benchmark_buffered(256),
    );

    println!();
    println!("Comparison:");
    print_comparison("Unbuffered", unbuffered, baseline, unbuffered);
    print_comparison("Buffered (64)", buffered_64, baseline, unbuffered);
    print_comparison("Buffered (128)", buffered_128, baseline, unbuffered);
    print_comparison("Buffered (256)", buffered_256, baseline, unbuffered);
}
