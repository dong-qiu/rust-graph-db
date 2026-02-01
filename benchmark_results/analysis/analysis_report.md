# Performance Analysis Report

**Generated**: 2026-02-01 10:33:43

---

## Executive Summary

- **Peak Read Throughput**: 2.58M ops/s
- **Peak Write Throughput**: 239.61K ops/s

## Criterion Benchmark Results

### vertex_scan

| Benchmark | Time (median) | Time Range |
|-----------|---------------|------------|
| 100 | 40.98 µs | [40.81 µs - 41.17 µs] |
| 1000 | 414.08 µs | [412.51 µs - 416.86 µs] |
| 10000 | 4.30 ms | [4.16 ms - 4.44 ms] |

### point_query

| Benchmark | Time (median) | Time Range |
|-----------|---------------|------------|
| 100 | 1.49 µs | [1.24 µs - 1.71 µs] |
| 1000 | 1.69 µs | [1.50 µs - 1.89 µs] |
| 10000 | 1.31 µs | [1.17 µs - 1.43 µs] |

### edge_traversal

| Benchmark | Time (median) | Time Range |
|-----------|---------------|------------|
| 100 | 3.64 µs | [3.25 µs - 3.92 µs] |
| 1000 | 3.83 µs | [3.34 µs - 4.18 µs] |
| 10000 | 4.21 µs | [3.87 µs - 4.46 µs] |

### vle

| Benchmark | Time (median) | Time Range |
|-----------|---------------|------------|
| depth/2 | 10.96 µs | [10.31 µs - 11.63 µs] |
| depth/3 | 19.17 µs | [16.38 µs - 20.77 µs] |
| depth/4 | 27.26 µs | [24.24 µs - 29.84 µs] |

### pattern_match_1hop

| Benchmark | Time (median) | Time Range |
|-----------|---------------|------------|
| 100 | 238.74 µs | [225.16 µs - 261.57 µs] |
| 1000 | 2.80 ms | [2.35 ms - 3.15 ms] |

### batch_create

| Benchmark | Time (median) | Time Range |
|-----------|---------------|------------|
| 10 | 2.99 ms | [2.53 ms - 3.30 ms] |
| 100 | 3.59 ms | [3.13 ms - 4.03 ms] |
| 1000 | 9.94 ms | [8.08 ms - 11.34 ms] |

### batch_edge_create

| Benchmark | Time (median) | Time Range |
|-----------|---------------|------------|
| 10 | 138.75 µs | [131.08 µs - 145.05 µs] |
| 100 | 1.18 ms | [1.13 ms - 1.31 ms] |
| 1000 | 13.94 ms | [11.82 ms - 15.55 ms] |

### scan_1000_vertices

| Benchmark | Time (median) | Time Range |
|-----------|---------------|------------|
| scan_1000_vertices | 300.67 µs | [297.92 µs - 305.83 µs] |

### create_edges

| Benchmark | Time (median) | Time Range |
|-----------|---------------|------------|
| batch_100 | 781.20 µs | [774.93 µs - 789.22 µs] |

### get_outgoing_edges

| Benchmark | Time (median) | Time Range |
|-----------|---------------|------------|
| get_outgoing_edges | 1.52 µs | [1.51 µs - 1.53 µs] |

## Concurrent Benchmark Results

### Read Workload

| Threads | Throughput | Latency p50 | Latency p99 | Total Ops |
|---------|------------|-------------|-------------|-----------|
| 1 | 1.00M ops/s | 0.001 ms | 0.001 ms | 10,017,357 |
| 4 | 1.96M ops/s | 0.002 ms | 0.004 ms | 19,552,822 |
| 8 | 2.38M ops/s | 0.002 ms | 0.004 ms | 23,767,942 |
| 16 | 2.58M ops/s | 0.002 ms | 0.004 ms | 51,605,738 |

### Write Workload

| Threads | Throughput | Latency p50 | Latency p99 | Total Ops |
|---------|------------|-------------|-------------|-----------|
| 4 | 239.61K ops/s | 0.011 ms | 0.058 ms | 2,396,157 |

### Mixed Workload

| Threads | Throughput | Latency p50 | Latency p99 | Total Ops |
|---------|------------|-------------|-------------|-----------|
| 4 | 1.08M ops/s | 0.002 ms | 0.023 ms | 10,809,830 |

## Scalability Analysis

### Read Workload Scaling

| Threads | Throughput | Expected (Linear) | Efficiency |
|---------|------------|-------------------|------------|
| 1 | 1.00M ops/s | 1.00M ops/s | 100.0% |
| 4 | 1.96M ops/s | 4.01M ops/s | 48.8% |
| 8 | 2.38M ops/s | 8.01M ops/s | 29.7% |
| 16 | 2.58M ops/s | 16.03M ops/s | 16.1% |

## Key Findings

- **Fastest operation**: point_query/10000 at 1.31 µs
- **Best concurrent read**: 2.58M ops/s with 16 threads
