use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use rust_graph_db::storage::rocksdb_store::RocksDbStorage;
use rust_graph_db::GraphStorage;
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// Benchmark creating vertices
fn bench_create_vertices(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let temp_dir = TempDir::new().unwrap();
    let storage = Arc::new(RocksDbStorage::new(temp_dir.path(), "bench_graph").unwrap());

    let mut group = c.benchmark_group("create_vertices");
    group.throughput(Throughput::Elements(1000));

    group.bench_function("batch_1000", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                let start = Instant::now();
                rt.block_on(async {
                    for i in 0..1000 {
                        let props = json!({
                            "name": format!("Person{}", i),
                            "age": 20 + (i % 60),
                        });
                        storage.create_vertex("Person", props).await.unwrap();
                    }
                    black_box(());
                });
                total += start.elapsed();
            }
            total
        });
    });

    group.finish();
}

/// Benchmark scanning vertices
fn bench_scan_vertices(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let temp_dir = TempDir::new().unwrap();
    let storage = Arc::new(RocksDbStorage::new(temp_dir.path(), "bench_graph").unwrap());

    // Pre-populate with 1000 vertices
    rt.block_on(async {
        for i in 0..1000 {
            let props = json!({
                "name": format!("Person{}", i),
                "age": 20 + (i % 60),
            });
            storage.create_vertex("Person", props).await.unwrap();
        }
    });

    c.bench_function("scan_1000_vertices", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                let start = Instant::now();
                rt.block_on(async {
                    let vertices = storage.scan_vertices("Person").await.unwrap();
                    black_box(vertices.len());
                });
                total += start.elapsed();
            }
            total
        });
    });
}

/// Benchmark creating edges
fn bench_create_edges(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let temp_dir = TempDir::new().unwrap();
    let storage = Arc::new(RocksDbStorage::new(temp_dir.path(), "bench_graph").unwrap());

    // Create vertices first
    let vertex_ids = rt.block_on(async {
        let mut ids = Vec::new();
        for i in 0..100 {
            let props = json!({"id": i});
            let v = storage.create_vertex("Node", props).await.unwrap();
            ids.push(v.id);
        }
        ids
    });

    let mut group = c.benchmark_group("create_edges");
    group.throughput(Throughput::Elements(100));

    group.bench_function("batch_100", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                let start = Instant::now();
                rt.block_on(async {
                    for i in 0..100 {
                        let props = json!({"weight": 1.0});
                        storage.create_edge(
                            "CONNECTS",
                            vertex_ids[i],
                            vertex_ids[(i + 1) % 100],
                            props
                        ).await.unwrap();
                    }
                    black_box(());
                });
                total += start.elapsed();
            }
            total
        });
    });

    group.finish();
}

/// Benchmark edge traversal
fn bench_edge_traversal(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let temp_dir = TempDir::new().unwrap();
    let storage = Arc::new(RocksDbStorage::new(temp_dir.path(), "bench_graph").unwrap());

    // Create a graph with edges
    let (start_vertex, _) = rt.block_on(async {
        let mut vertex_ids = Vec::new();
        for i in 0..100 {
            let props = json!({"id": i});
            let v = storage.create_vertex("Node", props).await.unwrap();
            vertex_ids.push(v.id);
        }

        // Create edges
        for i in 0..100 {
            let props = json!({"weight": 1.0});
            storage.create_edge(
                "CONNECTS",
                vertex_ids[i],
                vertex_ids[(i + 1) % 100],
                props
            ).await.unwrap();
        }

        (vertex_ids[0], vertex_ids)
    });

    c.bench_function("get_outgoing_edges", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                let start = Instant::now();
                rt.block_on(async {
                    let edges = storage.get_outgoing_edges(start_vertex).await.unwrap();
                    black_box(edges.len());
                });
                total += start.elapsed();
            }
            total
        });
    });
}

/// Benchmark shortest path
fn bench_shortest_path(c: &mut Criterion) {
    use rust_graph_db::algorithms::shortest_path;

    let rt = Runtime::new().unwrap();
    let temp_dir = TempDir::new().unwrap();
    let storage = Arc::new(RocksDbStorage::new(temp_dir.path(), "bench_graph").unwrap());

    // Create a 10x10 grid graph
    let (start_id, end_id) = rt.block_on(async {
        let size = 10;
        let mut grid = Vec::new();

        // Create vertices
        for row in 0..size {
            for col in 0..size {
                let props = json!({
                    "x": col,
                    "y": row,
                });
                let v = storage.create_vertex("GridNode", props).await.unwrap();
                grid.push(v.id);
            }
        }

        // Create edges (4-connected)
        for row in 0..size {
            for col in 0..size {
                let idx = row * size + col;
                let props = json!({"distance": 1.0});

                if col < size - 1 {
                    storage.create_edge("CONNECTED", grid[idx], grid[idx + 1], props.clone()).await.unwrap();
                }
                if row < size - 1 {
                    storage.create_edge("CONNECTED", grid[idx], grid[idx + size], props.clone()).await.unwrap();
                }
            }
        }

        (grid[0], grid[grid.len() - 1])
    });

    c.bench_function("shortest_path_10x10_grid", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                let start = Instant::now();
                rt.block_on(async {
                    let path = shortest_path(storage.clone(), start_id, end_id).await;
                    black_box(path);
                });
                total += start.elapsed();
            }
            total
        });
    });
}

criterion_group!(
    benches,
    bench_create_vertices,
    bench_scan_vertices,
    bench_create_edges,
    bench_edge_traversal,
    bench_shortest_path
);

criterion_main!(benches);
