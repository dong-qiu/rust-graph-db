#!/bin/bash
# Concurrent Benchmark Execution Script
# Tests scalability with different thread counts and workload types

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
RESULTS_DIR="${RESULTS_DIR:-${PROJECT_ROOT}/benchmark_results/concurrent}"
CARGO_BIN="${CARGO_BIN:-${HOME}/.cargo/bin/cargo}"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

# Test parameters
THREAD_COUNTS="${THREAD_COUNTS:-1 2 4 8 16}"
DURATION="${DURATION:-30}"  # seconds
INIT_VERTICES="${INIT_VERTICES:-10000}"

echo -e "${BLUE}=== Concurrent Benchmark Execution ===${NC}"
echo "Project Root: $PROJECT_ROOT"
echo "Results Directory: $RESULTS_DIR"
echo "Timestamp: $TIMESTAMP"
echo "Thread counts to test: $THREAD_COUNTS"
echo "Test duration: ${DURATION}s"
echo "Initial vertices: $INIT_VERTICES"
echo ""

# Create results directory
mkdir -p "$RESULTS_DIR"

# Function to run concurrent benchmark
run_concurrent_test() {
    local workload=$1
    local threads=$2
    local extra_args=$3

    local test_name="${workload}_${threads}threads"
    local output_json="$RESULTS_DIR/${test_name}_${TIMESTAMP}.json"
    local db_path="/tmp/concurrent_bench_${workload}_${threads}_$$"

    echo -e "${YELLOW}Testing: $workload workload with $threads threads${NC}"

    cd "$PROJECT_ROOT"
    $CARGO_BIN run --release --bin concurrent_bench -- \
        --workload "$workload" \
        --threads "$threads" \
        --duration "$DURATION" \
        --init-vertices "$INIT_VERTICES" \
        --db-path "$db_path" \
        --namespace "bench_${workload}" \
        --output "$output_json" \
        $extra_args

    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✓ $test_name completed${NC}"
        echo "  Results: $output_json"

        # Clean up test database
        rm -rf "$db_path"
    else
        echo -e "${RED}✗ $test_name failed${NC}"
        return 1
    fi
    echo ""
}

# Build concurrent benchmark tool
echo -e "${YELLOW}Building concurrent benchmark tool...${NC}"
cd "$PROJECT_ROOT"
$CARGO_BIN build --release --bin concurrent_bench

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Built successfully${NC}"
else
    echo -e "${RED}✗ Build failed${NC}"
    exit 1
fi
echo ""

# Run benchmarks
echo -e "${BLUE}--- Running Concurrent Benchmarks ---${NC}"
echo ""

# Test 1: Read workload scalability
echo -e "${BLUE}=== Read Workload Scalability ===${NC}"
for threads in $THREAD_COUNTS; do
    run_concurrent_test "read" "$threads"
    sleep 2  # Let system stabilize between tests
done

# Test 2: Write workload scalability
echo -e "${BLUE}=== Write Workload Scalability ===${NC}"
for threads in $THREAD_COUNTS; do
    run_concurrent_test "write" "$threads" "--init-vertices 0"
    sleep 2
done

# Test 3: Mixed workload (90/10 read/write ratio)
echo -e "${BLUE}=== Mixed Workload (90% read, 10% write) ===${NC}"
for threads in $THREAD_COUNTS; do
    run_concurrent_test "mixed" "$threads" "--read-ratio 0.9"
    sleep 2
done

# Test 4: Mixed workload (80/20 read/write ratio)
echo -e "${BLUE}=== Mixed Workload (80% read, 20% write) ===${NC}"
for threads in $THREAD_COUNTS; do
    run_concurrent_test "mixed" "$threads" "--read-ratio 0.8"
    sleep 2
done

# Generate summary
echo -e "${BLUE}--- Generating Summary ---${NC}"

# Count results
total_tests=$(ls -1 "$RESULTS_DIR"/*"$TIMESTAMP".json 2>/dev/null | wc -l)
echo "Total tests completed: $total_tests"
echo ""

# Extract key metrics from JSON files
echo -e "${BLUE}--- Performance Summary ---${NC}"
echo ""

for workload in read write mixed; do
    echo -e "${GREEN}${workload^} Workload:${NC}"

    files="$RESULTS_DIR/${workload}_*threads_${TIMESTAMP}.json"
    if ls $files 1> /dev/null 2>&1; then
        printf "%-10s %-15s %-15s %-15s %-15s\n" "Threads" "Throughput" "Latency p50" "Latency p95" "Latency p99"
        printf "%-10s %-15s %-15s %-15s %-15s\n" "-------" "---------" "-----------" "-----------" "-----------"

        for file in $files; do
            if [ -f "$file" ]; then
                threads=$(echo "$file" | grep -oP "${workload}_\K\d+(?=threads)")
                throughput=$(grep -oP '"throughput_ops_per_sec":\s*\K[0-9.]+' "$file" | head -1)
                p50=$(grep -oP '"p50":\s*\K[0-9.]+' "$file" | head -1)
                p95=$(grep -oP '"p95":\s*\K[0-9.]+' "$file" | head -1)
                p99=$(grep -oP '"p99":\s*\K[0-9.]+' "$file" | head -1)

                if [ -n "$throughput" ]; then
                    printf "%-10s %-15.0f %-15.3f %-15.3f %-15.3f\n" \
                        "$threads" "$throughput" "$p50" "$p95" "$p99"
                fi
            fi
        done
        echo ""
    fi
done

echo -e "${GREEN}=== Concurrent Benchmarks Complete ===${NC}"
echo ""
echo "Results directory: $RESULTS_DIR"
echo "Total JSON files: $total_tests"
echo ""
echo "Next steps:"
echo "  1. Analyze results: python3 scripts/analyze_results.py --input $RESULTS_DIR"
echo "  2. Generate charts: python3 scripts/generate_charts.py $RESULTS_DIR"
echo ""
