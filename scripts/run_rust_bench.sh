#!/bin/bash
# Rust Benchmark Execution Script
# Runs all Criterion benchmarks and saves results

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
RESULTS_DIR="${RESULTS_DIR:-${PROJECT_ROOT}/benchmark_results/rust}"
CARGO_BIN="${CARGO_BIN:-${HOME}/.cargo/bin/cargo}"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

echo -e "${BLUE}=== Rust Benchmark Execution ===${NC}"
echo "Project Root: $PROJECT_ROOT"
echo "Results Directory: $RESULTS_DIR"
echo "Timestamp: $TIMESTAMP"
echo ""

# Create results directory
mkdir -p "$RESULTS_DIR"

# Function to run benchmark
run_benchmark() {
    local bench_name=$1
    local description=$2

    echo -e "${YELLOW}Running $description...${NC}"

    # Run benchmark and save output
    local output_file="$RESULTS_DIR/${bench_name}_${TIMESTAMP}.txt"

    cd "$PROJECT_ROOT"
    $CARGO_BIN bench --bench "$bench_name" 2>&1 | tee "$output_file"

    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✓ $description completed${NC}"
        echo "  Results saved to: $output_file"
    else
        echo -e "${RED}✗ $description failed${NC}"
        return 1
    fi
    echo ""
}

# Clear page cache (requires sudo on Linux, use purge on macOS)
clear_cache() {
    echo -e "${YELLOW}Clearing system cache...${NC}"

    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS
        if command -v purge &> /dev/null; then
            sudo purge 2>/dev/null || purge 2>/dev/null || echo "Note: Could not clear cache (not critical)"
        fi
    elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
        # Linux
        sync
        echo 3 | sudo tee /proc/sys/vm/drop_caches > /dev/null 2>&1 || echo "Note: Could not clear cache (not critical)"
    fi

    echo ""
}

# Build benchmarks first
echo -e "${YELLOW}Building benchmarks in release mode...${NC}"
cd "$PROJECT_ROOT"
$CARGO_BIN build --release --benches

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Benchmarks built successfully${NC}"
else
    echo -e "${RED}✗ Failed to build benchmarks${NC}"
    exit 1
fi
echo ""

# Run benchmarks
echo -e "${BLUE}--- Running Benchmarks ---${NC}"
echo ""

# Optional: Clear cache before each benchmark
# clear_cache

# Run graph_ops benchmarks
run_benchmark "graph_ops" "Graph Operations Benchmark (CRUD, algorithms)"

# Sleep between benchmarks to let system stabilize
sleep 2

# Run query_ops benchmarks
run_benchmark "query_ops" "Query Operations Benchmark (scans, traversals, patterns)"

# Copy Criterion HTML reports
echo -e "${YELLOW}Copying Criterion HTML reports...${NC}"
if [ -d "$PROJECT_ROOT/target/criterion" ]; then
    cp -r "$PROJECT_ROOT/target/criterion" "$RESULTS_DIR/criterion_${TIMESTAMP}"
    echo -e "${GREEN}✓ HTML reports copied to: $RESULTS_DIR/criterion_${TIMESTAMP}${NC}"
else
    echo -e "${YELLOW}! No Criterion reports found${NC}"
fi
echo ""

# Generate summary
echo -e "${BLUE}--- Benchmark Summary ---${NC}"
echo "Results saved in: $RESULTS_DIR"
echo ""
echo "Files created:"
ls -lh "$RESULTS_DIR"/*"$TIMESTAMP"* | awk '{print "  " $9 " (" $5 ")"}'
echo ""

# Extract and display key metrics
echo -e "${BLUE}--- Quick Results Preview ---${NC}"
for file in "$RESULTS_DIR"/*"$TIMESTAMP".txt; do
    if [ -f "$file" ]; then
        echo -e "${GREEN}$(basename $file):${NC}"
        # Extract time measurements (look for "time:" patterns)
        grep -E "time:|slope|throughput" "$file" | head -10 || echo "  (See full results in file)"
        echo ""
    fi
done

echo -e "${GREEN}=== Rust Benchmarks Complete ===${NC}"
echo ""
echo "To view detailed HTML reports:"
echo "  open $RESULTS_DIR/criterion_${TIMESTAMP}/report/index.html"
echo ""
