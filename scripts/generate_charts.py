#!/usr/bin/env python3
"""
Performance Benchmark Chart Generator

Generates visualization charts from benchmark results:
- Throughput comparison charts
- Latency distribution charts
- Scalability charts
- Operation performance charts

Usage:
    python3 scripts/generate_charts.py --input benchmark_results/analysis --output charts/

Requirements:
    pip install matplotlib numpy
"""

import json
import argparse
import os
from pathlib import Path
from typing import Dict, List, Any, Optional
from datetime import datetime

try:
    import matplotlib.pyplot as plt
    import matplotlib.patches as mpatches
    import numpy as np
    HAS_MATPLOTLIB = True
except ImportError:
    HAS_MATPLOTLIB = False
    print("Warning: matplotlib not installed. Install with: pip install matplotlib numpy")


def load_analysis_data(input_dir: str) -> Dict[str, Any]:
    """Load analysis data from JSON file"""
    json_path = os.path.join(input_dir, 'analysis_data.json')
    if not os.path.exists(json_path):
        raise FileNotFoundError(f"Analysis data not found: {json_path}")

    with open(json_path, 'r') as f:
        return json.load(f)


def create_concurrent_throughput_chart(data: Dict[str, Any], output_dir: str):
    """Create concurrent throughput scalability chart"""
    concurrent = data.get('concurrent_benchmarks', [])
    if not concurrent:
        print("No concurrent benchmark data available")
        return

    # Group by workload type
    workloads = {}
    for item in concurrent:
        wl = item['workload'].lower()
        if wl not in workloads:
            workloads[wl] = []
        workloads[wl].append(item)

    fig, ax = plt.subplots(figsize=(10, 6))

    colors = {'read': '#2ecc71', 'write': '#e74c3c', 'mixed': '#3498db'}
    markers = {'read': 'o', 'write': 's', 'mixed': '^'}

    for workload, items in workloads.items():
        # Sort by threads
        items.sort(key=lambda x: x['threads'])
        threads = [item['threads'] for item in items]
        throughput = [item['throughput'] / 1_000_000 for item in items]  # Convert to M ops/s

        ax.plot(threads, throughput,
                marker=markers.get(workload, 'o'),
                color=colors.get(workload, '#333'),
                label=f'{workload.capitalize()} Workload',
                linewidth=2, markersize=8)

    ax.set_xlabel('Number of Threads', fontsize=12)
    ax.set_ylabel('Throughput (M ops/sec)', fontsize=12)
    ax.set_title('Concurrent Throughput Scalability', fontsize=14, fontweight='bold')
    ax.legend(loc='upper left')
    ax.grid(True, alpha=0.3)
    ax.set_xticks([1, 4, 8, 16])

    plt.tight_layout()
    output_path = os.path.join(output_dir, 'concurrent_throughput.png')
    plt.savefig(output_path, dpi=150)
    plt.close()
    print(f"Created: {output_path}")


def create_concurrent_latency_chart(data: Dict[str, Any], output_dir: str):
    """Create concurrent latency percentile chart"""
    concurrent = data.get('concurrent_benchmarks', [])
    if not concurrent:
        return

    # Filter read workload for latency comparison
    read_items = [item for item in concurrent if item['workload'].lower() == 'read']
    if not read_items:
        return

    read_items.sort(key=lambda x: x['threads'])

    fig, ax = plt.subplots(figsize=(10, 6))

    threads = [item['threads'] for item in read_items]
    p50 = [item['latency_p50_ms'] * 1000 for item in read_items]  # Convert to µs
    p99 = [item['latency_p99_ms'] * 1000 for item in read_items]

    x = np.arange(len(threads))
    width = 0.35

    bars1 = ax.bar(x - width/2, p50, width, label='p50 Latency', color='#3498db')
    bars2 = ax.bar(x + width/2, p99, width, label='p99 Latency', color='#e74c3c')

    ax.set_xlabel('Number of Threads', fontsize=12)
    ax.set_ylabel('Latency (µs)', fontsize=12)
    ax.set_title('Read Latency by Thread Count', fontsize=14, fontweight='bold')
    ax.set_xticks(x)
    ax.set_xticklabels(threads)
    ax.legend()
    ax.grid(True, alpha=0.3, axis='y')

    # Add value labels on bars
    for bar in bars1:
        height = bar.get_height()
        ax.annotate(f'{height:.1f}',
                    xy=(bar.get_x() + bar.get_width() / 2, height),
                    xytext=(0, 3), textcoords="offset points",
                    ha='center', va='bottom', fontsize=9)

    for bar in bars2:
        height = bar.get_height()
        ax.annotate(f'{height:.1f}',
                    xy=(bar.get_x() + bar.get_width() / 2, height),
                    xytext=(0, 3), textcoords="offset points",
                    ha='center', va='bottom', fontsize=9)

    plt.tight_layout()
    output_path = os.path.join(output_dir, 'concurrent_latency.png')
    plt.savefig(output_path, dpi=150)
    plt.close()
    print(f"Created: {output_path}")


def create_criterion_operations_chart(data: Dict[str, Any], output_dir: str):
    """Create Criterion benchmark operations chart"""
    criterion = data.get('criterion_benchmarks', {})
    if not criterion:
        print("No Criterion benchmark data available")
        return

    # Flatten all benchmarks
    all_benchmarks = []
    for file_results in criterion.values():
        all_benchmarks.extend(file_results)

    if not all_benchmarks:
        return

    # Group by category (first part of name)
    categories = {}
    for bench in all_benchmarks:
        name = bench['name']
        parts = name.split('/')
        category = parts[0]
        if category not in categories:
            categories[category] = []
        categories[category].append(bench)

    # Select key operations for chart
    key_ops = ['point_query', 'edge_traversal', 'vertex_scan', 'vle', 'shortest_path']

    fig, ax = plt.subplots(figsize=(12, 6))

    # Prepare data for chart
    labels = []
    times = []
    colors = []
    color_map = {
        'point_query': '#2ecc71',
        'edge_traversal': '#3498db',
        'vertex_scan': '#9b59b6',
        'vle': '#e74c3c',
        'shortest_path': '#f39c12',
        'pattern_match': '#1abc9c',
        'batch_create': '#34495e',
        'batch_edge': '#95a5a6'
    }

    for cat in key_ops:
        if cat in categories:
            for bench in categories[cat][:3]:  # Take up to 3 per category
                name = bench['name'].replace(f"{cat}/", "")
                if len(name) > 20:
                    name = name[:17] + "..."
                labels.append(f"{cat}\n{name}")

                # Convert to appropriate unit
                time_us = bench['time_mid_us']
                times.append(time_us)
                colors.append(color_map.get(cat, '#333'))

    if not labels:
        return

    y_pos = np.arange(len(labels))

    # Use log scale for better visualization
    ax.barh(y_pos, times, color=colors)
    ax.set_yticks(y_pos)
    ax.set_yticklabels(labels, fontsize=9)
    ax.set_xlabel('Time (µs) - Log Scale', fontsize=12)
    ax.set_title('Operation Performance Comparison', fontsize=14, fontweight='bold')
    ax.set_xscale('log')
    ax.grid(True, alpha=0.3, axis='x')

    # Add value labels
    for i, (time, label) in enumerate(zip(times, labels)):
        if time < 1:
            text = f'{time*1000:.0f}ns'
        elif time < 1000:
            text = f'{time:.1f}µs'
        else:
            text = f'{time/1000:.1f}ms'
        ax.annotate(text, xy=(time, i), xytext=(5, 0),
                    textcoords="offset points", va='center', fontsize=9)

    plt.tight_layout()
    output_path = os.path.join(output_dir, 'operation_performance.png')
    plt.savefig(output_path, dpi=150)
    plt.close()
    print(f"Created: {output_path}")


def create_scalability_efficiency_chart(data: Dict[str, Any], output_dir: str):
    """Create scalability efficiency chart"""
    concurrent = data.get('concurrent_benchmarks', [])
    if not concurrent:
        return

    read_items = [item for item in concurrent if item['workload'].lower() == 'read']
    if len(read_items) < 2:
        return

    read_items.sort(key=lambda x: x['threads'])

    # Calculate efficiency
    baseline = read_items[0]
    baseline_per_thread = baseline['throughput'] / baseline['threads']

    threads = []
    efficiency = []

    for item in read_items:
        expected = baseline_per_thread * item['threads']
        actual = item['throughput']
        eff = (actual / expected) * 100
        threads.append(item['threads'])
        efficiency.append(eff)

    fig, ax = plt.subplots(figsize=(10, 6))

    # Plot actual efficiency
    ax.plot(threads, efficiency, 'o-', color='#3498db', linewidth=2, markersize=10, label='Actual Efficiency')

    # Plot ideal (100%)
    ax.axhline(y=100, color='#2ecc71', linestyle='--', linewidth=2, label='Ideal (Linear Scaling)')

    # Fill area
    ax.fill_between(threads, efficiency, alpha=0.3, color='#3498db')

    ax.set_xlabel('Number of Threads', fontsize=12)
    ax.set_ylabel('Scaling Efficiency (%)', fontsize=12)
    ax.set_title('Read Workload Scaling Efficiency', fontsize=14, fontweight='bold')
    ax.legend(loc='upper right')
    ax.grid(True, alpha=0.3)
    ax.set_xticks(threads)
    ax.set_ylim(0, 120)

    # Add value labels
    for t, e in zip(threads, efficiency):
        ax.annotate(f'{e:.1f}%', xy=(t, e), xytext=(0, 10),
                    textcoords="offset points", ha='center', fontsize=10)

    plt.tight_layout()
    output_path = os.path.join(output_dir, 'scaling_efficiency.png')
    plt.savefig(output_path, dpi=150)
    plt.close()
    print(f"Created: {output_path}")


def create_summary_dashboard(data: Dict[str, Any], output_dir: str):
    """Create summary dashboard with key metrics"""
    fig, axes = plt.subplots(2, 2, figsize=(14, 10))

    concurrent = data.get('concurrent_benchmarks', [])
    criterion = data.get('criterion_benchmarks', {})

    # 1. Throughput by workload type (top-left)
    ax1 = axes[0, 0]
    workload_throughput = {}
    for item in concurrent:
        wl = item['workload'].lower()
        if wl not in workload_throughput:
            workload_throughput[wl] = []
        workload_throughput[wl].append(item['throughput'])

    if workload_throughput:
        labels = list(workload_throughput.keys())
        max_throughputs = [max(v)/1_000_000 for v in workload_throughput.values()]
        colors = ['#2ecc71', '#e74c3c', '#3498db'][:len(labels)]

        bars = ax1.bar(labels, max_throughputs, color=colors)
        ax1.set_ylabel('Peak Throughput (M ops/sec)')
        ax1.set_title('Peak Throughput by Workload Type', fontweight='bold')
        ax1.grid(True, alpha=0.3, axis='y')

        for bar, val in zip(bars, max_throughputs):
            ax1.annotate(f'{val:.2f}M', xy=(bar.get_x() + bar.get_width()/2, val),
                        xytext=(0, 5), textcoords="offset points", ha='center')

    # 2. Latency comparison (top-right)
    ax2 = axes[0, 1]
    read_items = [item for item in concurrent if item['workload'].lower() == 'read']
    if read_items:
        read_items.sort(key=lambda x: x['threads'])
        threads = [str(item['threads']) for item in read_items]
        p50 = [item['latency_p50_ms'] * 1000 for item in read_items]
        p99 = [item['latency_p99_ms'] * 1000 for item in read_items]

        x = np.arange(len(threads))
        width = 0.35
        ax2.bar(x - width/2, p50, width, label='p50', color='#3498db')
        ax2.bar(x + width/2, p99, width, label='p99', color='#e74c3c')
        ax2.set_xticks(x)
        ax2.set_xticklabels([f'{t} threads' for t in threads])
        ax2.set_ylabel('Latency (µs)')
        ax2.set_title('Read Latency Distribution', fontweight='bold')
        ax2.legend()
        ax2.grid(True, alpha=0.3, axis='y')

    # 3. Operation times (bottom-left)
    ax3 = axes[1, 0]
    all_benchmarks = []
    for file_results in criterion.values():
        all_benchmarks.extend(file_results)

    if all_benchmarks:
        # Select representative operations
        selected = {}
        for bench in all_benchmarks:
            cat = bench['name'].split('/')[0]
            if cat not in selected or '1000' in bench['name']:
                selected[cat] = bench

        names = []
        times = []
        for cat, bench in list(selected.items())[:8]:
            names.append(cat.replace('_', '\n'))
            times.append(bench['time_mid_us'])

        colors = plt.cm.viridis(np.linspace(0.2, 0.8, len(names)))
        ax3.barh(names, times, color=colors)
        ax3.set_xlabel('Time (µs) - Log Scale')
        ax3.set_title('Key Operation Performance', fontweight='bold')
        ax3.set_xscale('log')
        ax3.grid(True, alpha=0.3, axis='x')

    # 4. Scaling efficiency (bottom-right)
    ax4 = axes[1, 1]
    if len(read_items) >= 2:
        baseline = read_items[0]
        baseline_per_thread = baseline['throughput'] / baseline['threads']

        threads = []
        efficiency = []
        for item in read_items:
            expected = baseline_per_thread * item['threads']
            eff = (item['throughput'] / expected) * 100
            threads.append(item['threads'])
            efficiency.append(eff)

        ax4.plot(threads, efficiency, 'o-', color='#3498db', linewidth=2, markersize=8)
        ax4.axhline(y=100, color='#2ecc71', linestyle='--', alpha=0.7)
        ax4.fill_between(threads, efficiency, alpha=0.3, color='#3498db')
        ax4.set_xlabel('Number of Threads')
        ax4.set_ylabel('Scaling Efficiency (%)')
        ax4.set_title('Read Scaling Efficiency', fontweight='bold')
        ax4.set_xticks(threads)
        ax4.set_ylim(0, 120)
        ax4.grid(True, alpha=0.3)

    plt.suptitle('Rust Graph DB Performance Dashboard', fontsize=16, fontweight='bold', y=1.02)
    plt.tight_layout()

    output_path = os.path.join(output_dir, 'performance_dashboard.png')
    plt.savefig(output_path, dpi=150, bbox_inches='tight')
    plt.close()
    print(f"Created: {output_path}")


def generate_text_charts(data: Dict[str, Any], output_dir: str):
    """Generate ASCII text charts when matplotlib is not available"""
    concurrent = data.get('concurrent_benchmarks', [])

    output_lines = []
    output_lines.append("=" * 60)
    output_lines.append("PERFORMANCE CHARTS (TEXT MODE)")
    output_lines.append("=" * 60)
    output_lines.append("")

    # Throughput chart
    output_lines.append("Concurrent Throughput (M ops/sec)")
    output_lines.append("-" * 40)

    read_items = [item for item in concurrent if item['workload'].lower() == 'read']
    read_items.sort(key=lambda x: x['threads'])

    max_throughput = max(item['throughput'] for item in read_items) if read_items else 1

    for item in read_items:
        threads = item['threads']
        throughput = item['throughput'] / 1_000_000
        bar_len = int((item['throughput'] / max_throughput) * 30)
        bar = '#' * bar_len
        output_lines.append(f"{threads:2d} threads: {bar} {throughput:.2f}M")

    output_lines.append("")
    output_lines.append("Legend: # = relative throughput")

    # Save text charts
    output_path = os.path.join(output_dir, 'charts_text.txt')
    with open(output_path, 'w') as f:
        f.write('\n'.join(output_lines))
    print(f"Created: {output_path}")


def main():
    parser = argparse.ArgumentParser(description='Generate benchmark visualization charts')
    parser.add_argument('--input', type=str, default='benchmark_results/analysis',
                       help='Input directory containing analysis_data.json')
    parser.add_argument('--output', type=str, default='charts',
                       help='Output directory for chart images')

    args = parser.parse_args()

    # Create output directory
    os.makedirs(args.output, exist_ok=True)

    # Load data
    try:
        data = load_analysis_data(args.input)
        print(f"Loaded analysis data from {args.input}")
    except FileNotFoundError as e:
        print(f"Error: {e}")
        print("Run analyze_results.py first to generate analysis data")
        return

    if not HAS_MATPLOTLIB:
        print("\nGenerating text-based charts (matplotlib not available)...")
        generate_text_charts(data, args.output)
        return

    print("\nGenerating charts...")

    # Generate individual charts
    create_concurrent_throughput_chart(data, args.output)
    create_concurrent_latency_chart(data, args.output)
    create_criterion_operations_chart(data, args.output)
    create_scalability_efficiency_chart(data, args.output)

    # Generate dashboard
    create_summary_dashboard(data, args.output)

    print(f"\nAll charts saved to: {args.output}/")
    print("\nGenerated files:")
    for f in os.listdir(args.output):
        if f.endswith('.png') or f.endswith('.txt'):
            print(f"  - {f}")


if __name__ == '__main__':
    main()
