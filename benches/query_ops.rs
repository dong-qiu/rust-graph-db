use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rust_graph_db::storage::rocksdb_store::RocksDbStorage;
use rust_graph_db::GraphStorage;
use rust_graph_db::algorithms::{shortest_path, variable_length_expand, VleOptions};
use rust_graph_db::Graphid;
use serde_json::json;
use std::sync::Arc;
use std::time::Instant;
use tempfile::TempDir;

// Helper function to create test graph
async fn create_test_graph(size: usize) -> (Arc<RocksDbStorage>, Vec<Graphid>) {
    let temp_dir = TempDir::new().unwrap();
    let storage = Arc::new(
        RocksDbStorage::new(temp_dir.path().to_str().unwrap(), "bench").unwrap()
    );

    let mut vertex_ids = Vec::with_capacity(size);

    // Create vertices
    for i in 0..size {
        let props = json!({
            "id": i,
            "name": format!("Person{}", i),
            "age": 20 + (i % 60),
            "city": if i % 5 == 0 { "NYC" } else if i % 5 == 1 { "LA" } else if i % 5 == 2 { "Chicago" } else if i % 5 == 3 { "Houston" } else { "Phoenix" },
        });

        let vertex = storage.create_vertex("Person", props).await.unwrap();
        vertex_ids.push(vertex.id);
    }

    // Create edges (ring structure for shortest path)
    for i in 0..size {
        let next = (i + 1) % size;
        let props = json!({
            "since": 2020 + (i % 5),
            "weight": 1.0,
        });

        storage.create_edge(
            "KNOWS",
            vertex_ids[i],
            vertex_ids[next],
            props
        ).await.unwrap();
    }

    // Create additional random edges for more interesting graph structure
    for i in 0..size {
        if i % 10 == 0 && i + 5 < size {
            let props = json!({ "weight": 1.0 });
            storage.create_edge(
                "KNOWS",
                vertex_ids[i],
                vertex_ids[i + 5],
                props
            ).await.unwrap();
        }
    }

    // Leak temp_dir to keep it alive
    std::mem::forget(temp_dir);

    (storage, vertex_ids)
}

// Helper function to create grid graph for shortest path tests
async fn create_grid_graph(rows: usize, cols: usize) -> (Arc<RocksDbStorage>, Vec<Graphid>) {
    let temp_dir = TempDir::new().unwrap();
    let storage = Arc::new(
        RocksDbStorage::new(temp_dir.path().to_str().unwrap(), "grid_bench").unwrap()
    );

    let size = rows * cols;
    let mut vertex_ids = Vec::with_capacity(size);

    // Create vertices
    for row in 0..rows {
        for col in 0..cols {
            let props = json!({
                "x": col,
                "y": row,
                "name": format!("Node_{}_{}", row, col),
            });

            let vertex = storage.create_vertex("GridNode", props).await.unwrap();
            vertex_ids.push(vertex.id);
        }
    }

    // Create edges (4-connected grid)
    for row in 0..rows {
        for col in 0..cols {
            let idx = row * cols + col;
            let props = json!({ "distance": 1.0 });

            // Right neighbor
            if col < cols - 1 {
                storage.create_edge(
                    "CONNECTED",
                    vertex_ids[idx],
                    vertex_ids[idx + 1],
                    props.clone()
                ).await.unwrap();
            }

            // Bottom neighbor
            if row < rows - 1 {
                storage.create_edge(
                    "CONNECTED",
                    vertex_ids[idx],
                    vertex_ids[idx + cols],
                    props.clone()
                ).await.unwrap();
            }
        }
    }

    std::mem::forget(temp_dir);
    (storage, vertex_ids)
}

// Benchmark: Vertex scan by label
fn bench_vertex_scan(c: &mut Criterion) {
    let mut group = c.benchmark_group("vertex_scan");

    for size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let (storage, _) = runtime.block_on(create_test_graph(size));

            b.iter_custom(|iters| {
                let mut total = std::time::Duration::ZERO;
                for _ in 0..iters {
                    let start = Instant::now();
                    runtime.block_on(async {
                        let vertices = storage.scan_vertices("Person").await.unwrap();
                        black_box(vertices.len());
                    });
                    total += start.elapsed();
                }
                total
            });
        });
    }

    group.finish();
}

// Benchmark: Point query (get vertex by ID)
fn bench_point_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("point_query");

    for size in [100, 1000, 10000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let (storage, vertex_ids) = runtime.block_on(create_test_graph(size));
            let target_id = vertex_ids[size / 2];

            b.iter_custom(|iters| {
                let mut total = std::time::Duration::ZERO;
                for _ in 0..iters {
                    let start = Instant::now();
                    runtime.block_on(async {
                        let vertex = storage.get_vertex(target_id).await.unwrap();
                        black_box(vertex);
                    });
                    total += start.elapsed();
                }
                total
            });
        });
    }

    group.finish();
}

// Benchmark: Edge traversal (get outgoing edges)
fn bench_edge_traversal(c: &mut Criterion) {
    let mut group = c.benchmark_group("edge_traversal");

    for size in [100, 1000, 10000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let (storage, vertex_ids) = runtime.block_on(create_test_graph(size));
            let target_id = vertex_ids[0];

            b.iter_custom(|iters| {
                let mut total = std::time::Duration::ZERO;
                for _ in 0..iters {
                    let start = Instant::now();
                    runtime.block_on(async {
                        let edges = storage.get_outgoing_edges(target_id).await.unwrap();
                        black_box(edges.len());
                    });
                    total += start.elapsed();
                }
                total
            });
        });
    }

    group.finish();
}

// Benchmark: Shortest path algorithm
fn bench_shortest_path(c: &mut Criterion) {
    let mut group = c.benchmark_group("shortest_path");

    // Test on different grid sizes
    for size in [10, 20, 50].iter() {
        group.bench_with_input(
            BenchmarkId::new("grid", format!("{}x{}", size, size)),
            size,
            |b, &size| {
                let runtime = tokio::runtime::Runtime::new().unwrap();
                let (storage, vertex_ids) = runtime.block_on(create_grid_graph(size, size));
                let start_id = vertex_ids[0];
                let end_id = vertex_ids[vertex_ids.len() - 1];

                b.iter_custom(|iters| {
                    let mut total = std::time::Duration::ZERO;
                    for _ in 0..iters {
                        let start = Instant::now();
                        runtime.block_on(async {
                            let path = shortest_path(storage.clone(), start_id, end_id).await;
                            black_box(path);
                        });
                        total += start.elapsed();
                    }
                    total
                });
            },
        );
    }

    group.finish();
}

// Benchmark: Variable-length expansion (VLE)
fn bench_vle(c: &mut Criterion) {
    let mut group = c.benchmark_group("vle");

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (storage, vertex_ids) = runtime.block_on(create_test_graph(1000));
    let start_id = vertex_ids[0];

    for depth in [2, 3, 4].iter() {
        group.bench_with_input(
            BenchmarkId::new("depth", depth),
            depth,
            |b, &depth| {
                let opts = VleOptions {
                    min_length: 1,
                    max_length: depth,
                    allow_cycles: false,
                    max_paths: 1000,
                };

                b.iter_custom(|iters| {
                    let mut total = std::time::Duration::ZERO;
                    for _ in 0..iters {
                        let start = Instant::now();
                        runtime.block_on(async {
                            let paths = variable_length_expand(
                                storage.clone(),
                                start_id,
                                opts.clone()
                            ).await;
                            black_box(paths);
                        });
                        total += start.elapsed();
                    }
                    total
                });
            },
        );
    }

    group.finish();
}

// Benchmark: Pattern matching (1-hop)
fn bench_pattern_match_1hop(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_match_1hop");

    for size in [100, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let (storage, vertex_ids) = runtime.block_on(create_test_graph(size));

            b.iter_custom(|iters| {
                let mut total = std::time::Duration::ZERO;
                for _ in 0..iters {
                    let start = Instant::now();
                    runtime.block_on(async {
                        // Find all 1-hop relationships
                        let mut count = 0;
                        for vertex_id in &vertex_ids {
                            let edges = storage.get_outgoing_edges(*vertex_id).await.unwrap();
                            count += edges.len();
                        }
                        black_box(count);
                    });
                    total += start.elapsed();
                }
                total
            });
        });
    }

    group.finish();
}

// Benchmark: Batch vertex creation
fn bench_batch_create(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_create");

    for batch_size in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &batch_size| {
                let runtime = tokio::runtime::Runtime::new().unwrap();

                b.iter_custom(|iters| {
                    let mut total = std::time::Duration::ZERO;
                    for _ in 0..iters {
                        let start = Instant::now();
                        runtime.block_on(async {
                            let temp_dir = TempDir::new().unwrap();
                            let storage = RocksDbStorage::new(
                                temp_dir.path().to_str().unwrap(),
                                "batch_bench"
                            ).unwrap();

                            for i in 0..batch_size {
                                let props = json!({
                                    "id": i,
                                    "name": format!("Vertex{}", i),
                                });
                                storage.create_vertex("Node", props).await.unwrap();
                            }

                            black_box(batch_size);
                        });
                        total += start.elapsed();
                    }
                    total
                });
            },
        );
    }

    group.finish();
}

// Benchmark: Batch edge creation
fn bench_batch_edge_create(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_edge_create");

    for batch_size in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &batch_size| {
                let runtime = tokio::runtime::Runtime::new().unwrap();
                let (storage, vertex_ids) = runtime.block_on(create_test_graph(batch_size * 2));

                b.iter_custom(|iters| {
                    let mut total = std::time::Duration::ZERO;
                    for _ in 0..iters {
                        let start = Instant::now();
                        runtime.block_on(async {
                            for i in 0..batch_size {
                                let props = json!({ "weight": 1.0 });
                                storage.create_edge(
                                    "RELATES",
                                    vertex_ids[i],
                                    vertex_ids[i + batch_size],
                                    props
                                ).await.unwrap();
                            }
                            black_box(batch_size);
                        });
                        total += start.elapsed();
                    }
                    total
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    name = benches;
    config = Criterion::default()
        .sample_size(10)
        .measurement_time(std::time::Duration::from_secs(10));
    targets =
        bench_vertex_scan,
        bench_point_query,
        bench_edge_traversal,
        bench_shortest_path,
        bench_vle,
        bench_pattern_match_1hop,
        bench_batch_create,
        bench_batch_edge_create
);

criterion_main!(benches);
