#!/bin/bash
# Master Benchmark Orchestration Script
# Runs complete performance testing suite

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
RUN_ID="${RUN_ID:-run_${TIMESTAMP}}"

export RESULTS_DIR="${PROJECT_ROOT}/benchmark_results/${RUN_ID}"
export DATA_DIR="${PROJECT_ROOT}/benchmark_data"

echo -e "${CYAN}"
echo "╔══════════════════════════════════════════════════════╗"
echo "║   Performance Testing Suite: Rust vs C++ Graph DB   ║"
echo "╚══════════════════════════════════════════════════════╝"
echo -e "${NC}"
echo "Run ID: $RUN_ID"
echo "Project Root: $PROJECT_ROOT"
echo "Results Directory: $RESULTS_DIR"
echo "Data Directory: $DATA_DIR"
echo ""

# Create results directory structure
mkdir -p "$RESULTS_DIR"/{rust,cpp,concurrent,analysis,charts}

# Parse command line arguments
SKIP_DATA_GENERATION=false
SKIP_RUST_BENCH=false
SKIP_CPP_BENCH=false
SKIP_CONCURRENT=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --skip-data)
            SKIP_DATA_GENERATION=true
            shift
            ;;
        --skip-rust)
            SKIP_RUST_BENCH=true
            shift
            ;;
        --skip-cpp)
            SKIP_CPP_BENCH=true
            shift
            ;;
        --skip-concurrent)
            SKIP_CONCURRENT=true
            shift
            ;;
        --rust-only)
            SKIP_CPP_BENCH=true
            shift
            ;;
        --help)
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  --skip-data         Skip test data generation"
            echo "  --skip-rust         Skip Rust benchmarks"
            echo "  --skip-cpp          Skip C++ benchmarks"
            echo "  --skip-concurrent   Skip concurrent benchmarks"
            echo "  --rust-only         Only run Rust benchmarks (skip C++)"
            echo "  --help              Show this help message"
            echo ""
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Function to log step
log_step() {
    echo -e "${BLUE}┌─────────────────────────────────────────────────────┐${NC}"
    echo -e "${BLUE}│ $1${NC}"
    echo -e "${BLUE}└─────────────────────────────────────────────────────┘${NC}"
    echo ""
}

# Function to check if command succeeded
check_status() {
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✓ $1 completed successfully${NC}"
    else
        echo -e "${RED}✗ $1 failed${NC}"
        return 1
    fi
    echo ""
}

# Start time
START_TIME=$(date +%s)

# Phase 1: Test Data Generation
if [ "$SKIP_DATA_GENERATION" = false ]; then
    log_step "Phase 1: Generating Test Data"

    bash "$SCRIPT_DIR/setup_test_data.sh"
    check_status "Test data generation"
else
    echo -e "${YELLOW}Skipping test data generation${NC}"
    echo ""
fi

# Phase 2: Rust Benchmarks
if [ "$SKIP_RUST_BENCH" = false ]; then
    log_step "Phase 2: Running Rust Benchmarks"

    export RESULTS_DIR="$RESULTS_DIR/rust"
    bash "$SCRIPT_DIR/run_rust_bench.sh"
    check_status "Rust benchmarks"
else
    echo -e "${YELLOW}Skipping Rust benchmarks${NC}"
    echo ""
fi

# Phase 3: Concurrent Benchmarks
if [ "$SKIP_CONCURRENT" = false ]; then
    log_step "Phase 3: Running Concurrent Benchmarks"

    export RESULTS_DIR="$RESULTS_DIR/concurrent"
    export THREAD_COUNTS="1 2 4 8 16"
    export DURATION="30"
    bash "$SCRIPT_DIR/run_concurrent_bench.sh"
    check_status "Concurrent benchmarks"
else
    echo -e "${YELLOW}Skipping concurrent benchmarks${NC}"
    echo ""
fi

# Phase 4: C++ Benchmarks (if available)
if [ "$SKIP_CPP_BENCH" = false ]; then
    log_step "Phase 4: Running C++ Benchmarks"

    export RESULTS_DIR="$RESULTS_DIR/cpp"

    if bash "$SCRIPT_DIR/run_cpp_bench.sh"; then
        check_status "C++ benchmarks"
    else
        echo -e "${YELLOW}⚠ C++ benchmarks skipped (not set up)${NC}"
        echo ""
    fi
else
    echo -e "${YELLOW}Skipping C++ benchmarks${NC}"
    echo ""
fi

# Calculate execution time
END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))
HOURS=$((DURATION / 3600))
MINUTES=$(((DURATION % 3600) / 60))
SECONDS=$((DURATION % 60))

# Summary
echo -e "${CYAN}"
echo "╔══════════════════════════════════════════════════════╗"
echo "║              Benchmark Suite Complete                ║"
echo "╚══════════════════════════════════════════════════════╝"
echo -e "${NC}"
echo ""
echo -e "${GREEN}Results Summary:${NC}"
echo "  Run ID: $RUN_ID"
echo "  Total duration: ${HOURS}h ${MINUTES}m ${SECONDS}s"
echo "  Results location: $RESULTS_DIR"
echo ""

# List generated files
echo -e "${BLUE}Generated Files:${NC}"
find "$RESULTS_DIR" -type f \( -name "*.json" -o -name "*.txt" \) -exec ls -lh {} \; | \
    awk '{print "  " $9 " (" $5 ")"}'
echo ""

# Show directory sizes
echo -e "${BLUE}Directory Sizes:${NC}"
du -sh "$RESULTS_DIR"/* 2>/dev/null | awk '{print "  " $2 ": " $1}' || echo "  (no results)"
echo ""

# Next steps
echo -e "${YELLOW}Next Steps:${NC}"
echo "  1. Analyze results:"
echo "     python3 scripts/analyze_results.py --rust $RESULTS_DIR/rust --concurrent $RESULTS_DIR/concurrent"
echo ""
echo "  2. Generate comparison charts:"
echo "     python3 scripts/generate_charts.py $RESULTS_DIR/analysis charts/"
echo ""
echo "  3. Generate final report:"
echo "     python3 scripts/generate_report.py $RESULTS_DIR PERFORMANCE_COMPARISON_REPORT.md"
echo ""
echo "  4. View Criterion HTML reports:"
echo "     open $RESULTS_DIR/rust/criterion_*/report/index.html"
echo ""

echo -e "${GREEN}✓ All benchmarks completed successfully!${NC}"
