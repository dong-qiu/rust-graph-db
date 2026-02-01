#!/usr/bin/env python3
"""
Performance Benchmark Results Analyzer

Analyzes and summarizes benchmark results from:
- Criterion benchmarks (graph_ops, query_ops)
- Concurrent benchmarks (read, write, mixed)

Usage:
    python3 scripts/analyze_results.py --rust benchmark_results/rust --concurrent benchmark_results/concurrent
"""

import json
import re
import argparse
import os
from pathlib import Path
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Any
from datetime import datetime


@dataclass
class CriterionResult:
    """Parsed Criterion benchmark result"""
    name: str
    time_low: float  # microseconds
    time_mid: float  # microseconds
    time_high: float  # microseconds
    throughput: Optional[str] = None


@dataclass
class ConcurrentResult:
    """Parsed concurrent benchmark result"""
    workload_type: str
    threads: int
    duration_secs: int
    total_operations: int
    successful_operations: int
    failed_operations: int
    throughput_ops_per_sec: float
    latency_min: float
    latency_max: float
    latency_mean: float
    latency_p50: float
    latency_p95: float
    latency_p99: float


def parse_criterion_output(file_path: str) -> List[CriterionResult]:
    """Parse Criterion benchmark output file"""
    results = []

    with open(file_path, 'r') as f:
        content = f.read()

    # Pattern to match benchmark results
    # Example: vertex_scan/100         time:   [40.809 µs 40.979 µs 41.166 µs]
    pattern = r'^(\S+)\s+time:\s+\[(\d+\.?\d*)\s+(µs|ms|ns)\s+(\d+\.?\d*)\s+(µs|ms|ns)\s+(\d+\.?\d*)\s+(µs|ms|ns)\]'

    # Also capture throughput if present
    throughput_pattern = r'^\s+thrpt:\s+\[.+\]'

    lines = content.split('\n')
    i = 0
    while i < len(lines):
        line = lines[i]
        match = re.match(pattern, line)
        if match:
            name = match.group(1)
            time_low = float(match.group(2))
            unit_low = match.group(3)
            time_mid = float(match.group(4))
            unit_mid = match.group(5)
            time_high = float(match.group(6))
            unit_high = match.group(7)

            # Convert to microseconds
            def to_us(value: float, unit: str) -> float:
                if unit == 'ns':
                    return value / 1000
                elif unit == 'ms':
                    return value * 1000
                return value  # already µs

            time_low_us = to_us(time_low, unit_low)
            time_mid_us = to_us(time_mid, unit_mid)
            time_high_us = to_us(time_high, unit_high)

            # Check for throughput on next line
            throughput = None
            if i + 1 < len(lines) and 'thrpt:' in lines[i + 1]:
                throughput = lines[i + 1].strip()

            results.append(CriterionResult(
                name=name,
                time_low=time_low_us,
                time_mid=time_mid_us,
                time_high=time_high_us,
                throughput=throughput
            ))
        i += 1

    return results


def parse_concurrent_json(file_path: str) -> ConcurrentResult:
    """Parse concurrent benchmark JSON file"""
    with open(file_path, 'r') as f:
        data = json.load(f)

    return ConcurrentResult(
        workload_type=data['workload_type'],
        threads=data['threads'],
        duration_secs=data['duration_secs'],
        total_operations=data['total_operations'],
        successful_operations=data['successful_operations'],
        failed_operations=data['failed_operations'],
        throughput_ops_per_sec=data['throughput_ops_per_sec'],
        latency_min=data['latencies_ms']['min'],
        latency_max=data['latencies_ms']['max'],
        latency_mean=data['latencies_ms']['mean'],
        latency_p50=data['latencies_ms']['p50'],
        latency_p95=data['latencies_ms']['p95'],
        latency_p99=data['latencies_ms']['p99']
    )


def format_time(us: float) -> str:
    """Format time in appropriate units"""
    if us < 1:
        return f"{us * 1000:.2f} ns"
    elif us < 1000:
        return f"{us:.2f} µs"
    elif us < 1000000:
        return f"{us / 1000:.2f} ms"
    else:
        return f"{us / 1000000:.2f} s"


def format_throughput(ops: float) -> str:
    """Format throughput with appropriate suffix"""
    if ops >= 1000000:
        return f"{ops / 1000000:.2f}M ops/s"
    elif ops >= 1000:
        return f"{ops / 1000:.2f}K ops/s"
    else:
        return f"{ops:.2f} ops/s"


def generate_criterion_summary(results: List[CriterionResult]) -> str:
    """Generate summary for Criterion benchmarks"""
    lines = []
    lines.append("## Criterion Benchmark Results\n")

    # Group results by category
    categories = {}
    for r in results:
        parts = r.name.split('/')
        category = parts[0]
        if category not in categories:
            categories[category] = []
        categories[category].append(r)

    for category, cat_results in categories.items():
        lines.append(f"### {category}\n")
        lines.append("| Benchmark | Time (median) | Time Range |")
        lines.append("|-----------|---------------|------------|")

        for r in cat_results:
            name = r.name.replace(f"{category}/", "")
            lines.append(f"| {name} | {format_time(r.time_mid)} | [{format_time(r.time_low)} - {format_time(r.time_high)}] |")

        lines.append("")

    return "\n".join(lines)


def generate_concurrent_summary(results: List[ConcurrentResult]) -> str:
    """Generate summary for concurrent benchmarks"""
    lines = []
    lines.append("## Concurrent Benchmark Results\n")

    # Group by workload type
    workloads = {}
    for r in results:
        wt = r.workload_type.lower()
        if wt not in workloads:
            workloads[wt] = []
        workloads[wt].append(r)

    for workload, wl_results in workloads.items():
        lines.append(f"### {workload.capitalize()} Workload\n")
        lines.append("| Threads | Throughput | Latency p50 | Latency p99 | Total Ops |")
        lines.append("|---------|------------|-------------|-------------|-----------|")

        # Sort by threads
        wl_results.sort(key=lambda x: x.threads)

        for r in wl_results:
            lines.append(
                f"| {r.threads} | {format_throughput(r.throughput_ops_per_sec)} | "
                f"{r.latency_p50:.3f} ms | {r.latency_p99:.3f} ms | {r.total_operations:,} |"
            )

        lines.append("")

    return "\n".join(lines)


def calculate_scalability_metrics(results: List[ConcurrentResult]) -> Dict[str, Any]:
    """Calculate scalability metrics for concurrent benchmarks"""
    metrics = {}

    # Group by workload type
    workloads = {}
    for r in results:
        wt = r.workload_type.lower()
        if wt not in workloads:
            workloads[wt] = []
        workloads[wt].append(r)

    for workload, wl_results in workloads.items():
        wl_results.sort(key=lambda x: x.threads)

        if len(wl_results) < 2:
            continue

        # Calculate scaling efficiency
        baseline = next((r for r in wl_results if r.threads == 1), wl_results[0])
        baseline_throughput = baseline.throughput_ops_per_sec / baseline.threads

        scaling_data = []
        for r in wl_results:
            expected_linear = baseline_throughput * r.threads
            actual = r.throughput_ops_per_sec
            efficiency = (actual / expected_linear) * 100 if expected_linear > 0 else 0
            scaling_data.append({
                'threads': r.threads,
                'throughput': actual,
                'expected_linear': expected_linear,
                'efficiency': efficiency
            })

        metrics[workload] = scaling_data

    return metrics


def generate_analysis_report(
    criterion_results: Dict[str, List[CriterionResult]],
    concurrent_results: List[ConcurrentResult],
    output_dir: str
) -> str:
    """Generate comprehensive analysis report"""
    lines = []
    timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")

    lines.append("# Performance Analysis Report")
    lines.append(f"\n**Generated**: {timestamp}\n")
    lines.append("---\n")

    # Executive Summary
    lines.append("## Executive Summary\n")

    # Calculate key metrics
    if concurrent_results:
        read_results = [r for r in concurrent_results if r.workload_type.lower() == 'read']
        write_results = [r for r in concurrent_results if r.workload_type.lower() == 'write']

        if read_results:
            max_read = max(r.throughput_ops_per_sec for r in read_results)
            lines.append(f"- **Peak Read Throughput**: {format_throughput(max_read)}")

        if write_results:
            max_write = max(r.throughput_ops_per_sec for r in write_results)
            lines.append(f"- **Peak Write Throughput**: {format_throughput(max_write)}")

    lines.append("")

    # Criterion benchmarks
    all_criterion = []
    for results in criterion_results.values():
        all_criterion.extend(results)

    if all_criterion:
        lines.append(generate_criterion_summary(all_criterion))

    # Concurrent benchmarks
    if concurrent_results:
        lines.append(generate_concurrent_summary(concurrent_results))

        # Scalability analysis
        scalability = calculate_scalability_metrics(concurrent_results)
        if scalability:
            lines.append("## Scalability Analysis\n")
            for workload, data in scalability.items():
                lines.append(f"### {workload.capitalize()} Workload Scaling\n")
                lines.append("| Threads | Throughput | Expected (Linear) | Efficiency |")
                lines.append("|---------|------------|-------------------|------------|")
                for d in data:
                    lines.append(
                        f"| {d['threads']} | {format_throughput(d['throughput'])} | "
                        f"{format_throughput(d['expected_linear'])} | {d['efficiency']:.1f}% |"
                    )
                lines.append("")

    # Performance Highlights
    lines.append("## Key Findings\n")

    # Find best performers
    if all_criterion:
        fastest = min(all_criterion, key=lambda x: x.time_mid)
        lines.append(f"- **Fastest operation**: {fastest.name} at {format_time(fastest.time_mid)}")

    if concurrent_results:
        best_read = max(
            [r for r in concurrent_results if r.workload_type.lower() == 'read'],
            key=lambda x: x.throughput_ops_per_sec,
            default=None
        )
        if best_read:
            lines.append(
                f"- **Best concurrent read**: {format_throughput(best_read.throughput_ops_per_sec)} "
                f"with {best_read.threads} threads"
            )

    lines.append("")

    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description='Analyze benchmark results')
    parser.add_argument('--rust', type=str, help='Directory containing Rust/Criterion results')
    parser.add_argument('--concurrent', type=str, help='Directory containing concurrent benchmark JSON files')
    parser.add_argument('--output', type=str, default='benchmark_results/analysis',
                       help='Output directory for analysis results')

    args = parser.parse_args()

    criterion_results = {}
    concurrent_results = []

    # Parse Criterion results
    if args.rust and os.path.isdir(args.rust):
        for file in os.listdir(args.rust):
            if file.endswith('.txt'):
                file_path = os.path.join(args.rust, file)
                results = parse_criterion_output(file_path)
                if results:
                    criterion_results[file] = results
                    print(f"Parsed {len(results)} benchmarks from {file}")

    # Parse concurrent results
    if args.concurrent and os.path.isdir(args.concurrent):
        for file in os.listdir(args.concurrent):
            if file.endswith('.json'):
                file_path = os.path.join(args.concurrent, file)
                try:
                    result = parse_concurrent_json(file_path)
                    concurrent_results.append(result)
                    print(f"Parsed concurrent result from {file}")
                except Exception as e:
                    print(f"Error parsing {file}: {e}")

    # Generate report
    os.makedirs(args.output, exist_ok=True)

    report = generate_analysis_report(criterion_results, concurrent_results, args.output)

    report_path = os.path.join(args.output, 'analysis_report.md')
    with open(report_path, 'w') as f:
        f.write(report)
    print(f"\nAnalysis report written to: {report_path}")

    # Also save structured data as JSON
    summary_data = {
        'timestamp': datetime.now().isoformat(),
        'criterion_benchmarks': {
            name: [
                {
                    'name': r.name,
                    'time_mid_us': r.time_mid,
                    'time_low_us': r.time_low,
                    'time_high_us': r.time_high
                }
                for r in results
            ]
            for name, results in criterion_results.items()
        },
        'concurrent_benchmarks': [
            {
                'workload': r.workload_type,
                'threads': r.threads,
                'throughput': r.throughput_ops_per_sec,
                'latency_p50_ms': r.latency_p50,
                'latency_p99_ms': r.latency_p99,
                'total_ops': r.total_operations
            }
            for r in concurrent_results
        ]
    }

    json_path = os.path.join(args.output, 'analysis_data.json')
    with open(json_path, 'w') as f:
        json.dump(summary_data, f, indent=2)
    print(f"Analysis data written to: {json_path}")

    # Print summary to console
    print("\n" + "=" * 60)
    print("ANALYSIS SUMMARY")
    print("=" * 60)
    print(report)


if __name__ == '__main__':
    main()
