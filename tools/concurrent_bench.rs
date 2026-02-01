use clap::{Parser, ValueEnum};
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::task::JoinSet;
use rust_graph_db::storage::rocksdb_store::RocksDbStorage;
use rust_graph_db::GraphStorage;
use serde_json::json;
use anyhow::Result;

#[derive(Debug, Clone, ValueEnum)]
enum WorkloadType {
    Read,
    Write,
    Mixed,
}

#[derive(Parser, Debug)]
#[command(name = "concurrent_bench")]
#[command(about = "Concurrent benchmark tool for graph database", long_about = None)]
struct Args {
    /// Workload type
    #[arg(short, long, value_enum)]
    workload: WorkloadType,

    /// Number of threads
    #[arg(short = 't', long, default_value_t = 4)]
    threads: usize,

    /// Duration in seconds
    #[arg(short, long, default_value_t = 30)]
    duration: u64,

    /// Database path
    #[arg(short = 'p', long, default_value = "./data/concurrent_bench")]
    db_path: String,

    /// Database namespace
    #[arg(short = 'n', long, default_value = "benchmark")]
    namespace: String,

    /// Number of pre-existing vertices (0 to skip initialization)
    #[arg(short = 'v', long, default_value_t = 10000)]
    init_vertices: usize,

    /// Read/write ratio for mixed workload (0.0-1.0, where 0.9 means 90% reads)
    #[arg(short, long, default_value_t = 0.9)]
    read_ratio: f64,

    /// Output JSON results to file
    #[arg(short, long)]
    output: Option<String>,

    /// Random seed
    #[arg(long, default_value_t = 42)]
    seed: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct BenchmarkResults {
    workload_type: String,
    threads: usize,
    duration_secs: u64,
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    throughput_ops_per_sec: f64,
    latencies_ms: LatencyStats,
    per_thread_ops: Vec<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct LatencyStats {
    min: f64,
    max: f64,
    mean: f64,
    p50: f64,
    p95: f64,
    p99: f64,
}

struct WorkerStats {
    operations: AtomicU64,
    successes: AtomicU64,
    failures: AtomicU64,
    latencies: parking_lot::Mutex<Vec<Duration>>,
}

impl WorkerStats {
    fn new() -> Self {
        Self {
            operations: AtomicU64::new(0),
            successes: AtomicU64::new(0),
            failures: AtomicU64::new(0),
            latencies: parking_lot::Mutex::new(Vec::new()),
        }
    }

    fn record_success(&self, latency: Duration) {
        self.operations.fetch_add(1, Ordering::Relaxed);
        self.successes.fetch_add(1, Ordering::Relaxed);
        self.latencies.lock().push(latency);
    }

    fn record_failure(&self) {
        self.operations.fetch_add(1, Ordering::Relaxed);
        self.failures.fetch_add(1, Ordering::Relaxed);
    }
}

async fn initialize_database(
    storage: Arc<dyn GraphStorage>,
    num_vertices: usize,
) -> Result<Vec<rust_graph_db::Graphid>> {
    println!("Initializing database with {} vertices...", num_vertices);

    let mut vertex_ids = Vec::with_capacity(num_vertices);

    // Create vertices in batches
    let batch_size = 1000;
    for batch_start in (0..num_vertices).step_by(batch_size) {
        let batch_end = (batch_start + batch_size).min(num_vertices);

        for i in batch_start..batch_end {
            let props = json!({
                "id": i,
                "name": format!("Vertex{}", i),
                "value": i * 2,
            });

            let vertex = storage.create_vertex("Node", props).await?;
            vertex_ids.push(vertex.id);
        }

        if batch_end % 10000 == 0 {
            println!("  Created {} vertices...", batch_end);
        }
    }

    println!("Database initialization complete!");
    Ok(vertex_ids)
}

async fn run_read_worker(
    storage: Arc<dyn GraphStorage>,
    vertex_ids: Arc<Vec<rust_graph_db::Graphid>>,
    duration: Duration,
    stats: Arc<WorkerStats>,
    thread_id: usize,
    seed: u64,
) {
    let mut rng = StdRng::seed_from_u64(seed + thread_id as u64);
    let start = Instant::now();
    let mut local_ops = 0u64;

    while start.elapsed() < duration {
        // Random read operation
        let idx = rng.gen_range(0..vertex_ids.len());
        let vertex_id = vertex_ids[idx];

        let op_start = Instant::now();
        match storage.get_vertex(vertex_id).await {
            Ok(Some(_)) => {
                stats.record_success(op_start.elapsed());
                local_ops += 1;
            }
            Ok(None) => {
                stats.record_failure();
            }
            Err(_) => {
                stats.record_failure();
            }
        }
    }

    println!("Thread {} completed {} read operations", thread_id, local_ops);
}

async fn run_write_worker(
    storage: Arc<dyn GraphStorage>,
    duration: Duration,
    stats: Arc<WorkerStats>,
    thread_id: usize,
    seed: u64,
) {
    let mut rng = StdRng::seed_from_u64(seed + thread_id as u64);
    let start = Instant::now();
    let mut local_ops = 0u64;

    while start.elapsed() < duration {
        // Create new vertex
        let vertex_id = rng.gen::<u64>();
        let props = json!({
            "thread_id": thread_id,
            "vertex_id": vertex_id,
            "name": format!("NewVertex_{}_{}", thread_id, vertex_id),
            "timestamp": start.elapsed().as_millis(),
        });

        let op_start = Instant::now();
        match storage.create_vertex("NewNode", props).await {
            Ok(_) => {
                stats.record_success(op_start.elapsed());
                local_ops += 1;
            }
            Err(_) => {
                stats.record_failure();
            }
        }
    }

    println!("Thread {} completed {} write operations", thread_id, local_ops);
}

async fn run_mixed_worker(
    storage: Arc<dyn GraphStorage>,
    vertex_ids: Arc<Vec<rust_graph_db::Graphid>>,
    duration: Duration,
    stats: Arc<WorkerStats>,
    thread_id: usize,
    read_ratio: f64,
    seed: u64,
) {
    let mut rng = StdRng::seed_from_u64(seed + thread_id as u64);
    let start = Instant::now();
    let mut local_reads = 0u64;
    let mut local_writes = 0u64;

    while start.elapsed() < duration {
        let op_start = Instant::now();

        if rng.gen::<f64>() < read_ratio {
            // Read operation
            let idx = rng.gen_range(0..vertex_ids.len());
            let vertex_id = vertex_ids[idx];

            match storage.get_vertex(vertex_id).await {
                Ok(Some(_)) => {
                    stats.record_success(op_start.elapsed());
                    local_reads += 1;
                }
                Ok(None) => {
                    stats.record_failure();
                }
                Err(_) => {
                    stats.record_failure();
                }
            }
        } else {
            // Write operation
            let vertex_id = rng.gen::<u64>();
            let props = json!({
                "thread_id": thread_id,
                "vertex_id": vertex_id,
                "name": format!("MixedVertex_{}_{}", thread_id, vertex_id),
            });

            match storage.create_vertex("MixedNode", props).await {
                Ok(_) => {
                    stats.record_success(op_start.elapsed());
                    local_writes += 1;
                }
                Err(_) => {
                    stats.record_failure();
                }
            }
        }
    }

    println!(
        "Thread {} completed {} reads, {} writes",
        thread_id, local_reads, local_writes
    );
}

fn calculate_latency_stats(latencies: &[Duration]) -> LatencyStats {
    if latencies.is_empty() {
        return LatencyStats {
            min: 0.0,
            max: 0.0,
            mean: 0.0,
            p50: 0.0,
            p95: 0.0,
            p99: 0.0,
        };
    }

    let mut sorted: Vec<f64> = latencies
        .iter()
        .map(|d| d.as_secs_f64() * 1000.0)
        .collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let len = sorted.len();
    let sum: f64 = sorted.iter().sum();

    LatencyStats {
        min: sorted[0],
        max: sorted[len - 1],
        mean: sum / len as f64,
        p50: sorted[len / 2],
        p95: sorted[(len * 95) / 100],
        p99: sorted[(len * 99) / 100],
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    println!("=== Concurrent Benchmark Configuration ===");
    println!("Workload: {:?}", args.workload);
    println!("Threads: {}", args.threads);
    println!("Duration: {}s", args.duration);
    println!("Database: {} (namespace: {})", args.db_path, args.namespace);
    println!("==========================================\n");

    // Initialize storage
    let storage = Arc::new(RocksDbStorage::new(&args.db_path, &args.namespace)?);

    // Initialize database with vertices for read operations
    let vertex_ids = if args.init_vertices > 0 {
        Arc::new(initialize_database(storage.clone(), args.init_vertices).await?)
    } else {
        Arc::new(Vec::new())
    };

    println!("\nStarting benchmark...\n");

    // Create shared stats
    let stats = Arc::new(WorkerStats::new());
    let duration = Duration::from_secs(args.duration);

    // Spawn worker tasks
    let mut join_set = JoinSet::new();
    let start_time = Instant::now();

    for thread_id in 0..args.threads {
        let storage_clone = storage.clone();
        let vertex_ids_clone = vertex_ids.clone();
        let stats_clone = stats.clone();
        let workload = args.workload.clone();
        let read_ratio = args.read_ratio;
        let seed = args.seed;

        join_set.spawn(async move {
            match workload {
                WorkloadType::Read => {
                    run_read_worker(
                        storage_clone,
                        vertex_ids_clone,
                        duration,
                        stats_clone,
                        thread_id,
                        seed,
                    )
                    .await;
                }
                WorkloadType::Write => {
                    run_write_worker(storage_clone, duration, stats_clone, thread_id, seed).await;
                }
                WorkloadType::Mixed => {
                    run_mixed_worker(
                        storage_clone,
                        vertex_ids_clone,
                        duration,
                        stats_clone,
                        thread_id,
                        read_ratio,
                        seed,
                    )
                    .await;
                }
            }
            thread_id
        });
    }

    // Wait for all workers to complete
    let mut per_thread_ops = vec![0u64; args.threads];
    while let Some(result) = join_set.join_next().await {
        if let Ok(_thread_id) = result {
            // Thread completed
        }
    }

    let actual_duration = start_time.elapsed();

    // Collect results
    let total_ops = stats.operations.load(Ordering::Relaxed);
    let successes = stats.successes.load(Ordering::Relaxed);
    let failures = stats.failures.load(Ordering::Relaxed);
    let latencies = stats.latencies.lock().clone();

    let throughput = successes as f64 / actual_duration.as_secs_f64();
    let latency_stats = calculate_latency_stats(&latencies);

    // Calculate per-thread operations (approximate)
    let avg_per_thread = total_ops / args.threads as u64;
    for i in 0..args.threads {
        per_thread_ops[i] = avg_per_thread;
    }

    let results = BenchmarkResults {
        workload_type: format!("{:?}", args.workload),
        threads: args.threads,
        duration_secs: actual_duration.as_secs(),
        total_operations: total_ops,
        successful_operations: successes,
        failed_operations: failures,
        throughput_ops_per_sec: throughput,
        latencies_ms: latency_stats,
        per_thread_ops,
    };

    // Print results
    println!("\n=== Benchmark Results ===");
    println!("Total operations: {}", results.total_operations);
    println!("Successful: {}", results.successful_operations);
    println!("Failed: {}", results.failed_operations);
    println!("Duration: {:.2}s", actual_duration.as_secs_f64());
    println!("Throughput: {:.2} ops/sec", results.throughput_ops_per_sec);
    println!("\nLatency (ms):");
    println!("  Min: {:.3}", results.latencies_ms.min);
    println!("  Mean: {:.3}", results.latencies_ms.mean);
    println!("  P50: {:.3}", results.latencies_ms.p50);
    println!("  P95: {:.3}", results.latencies_ms.p95);
    println!("  P99: {:.3}", results.latencies_ms.p99);
    println!("  Max: {:.3}", results.latencies_ms.max);

    // Save to JSON if requested
    if let Some(output_path) = args.output {
        let json_output = serde_json::to_string_pretty(&results)?;
        std::fs::write(&output_path, json_output)?;
        println!("\n✅ Results saved to: {}", output_path);
    }

    Ok(())
}
