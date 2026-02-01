#!/bin/bash
# Test Data Generation Script
# Generates various graph datasets for performance testing

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
DATA_DIR="${DATA_DIR:-${PROJECT_ROOT}/benchmark_data}"
CARGO_BIN="${CARGO_BIN:-${HOME}/.cargo/bin/cargo}"

echo -e "${BLUE}=== Test Data Generation ===${NC}"
echo "Project Root: $PROJECT_ROOT"
echo "Data Directory: $DATA_DIR"
echo ""

# Create data directory
mkdir -p "$DATA_DIR"

# Function to generate dataset
generate_dataset() {
    local name=$1
    local graph_type=$2
    local vertices=$3
    local extra_args=$4

    echo -e "${YELLOW}Generating $name dataset...${NC}"

    $CARGO_BIN run --release --bin data_generator -- \
        --graph-type "$graph_type" \
        --vertices "$vertices" \
        --output "$DATA_DIR/$name" \
        --formats json,csv \
        --seed 42 \
        $extra_args

    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✓ $name dataset created${NC}"
    else
        echo -e "${RED}✗ Failed to create $name dataset${NC}"
        return 1
    fi
    echo ""
}

# Small datasets (for quick tests)
echo -e "${BLUE}--- Small Datasets (Quick Tests) ---${NC}"
generate_dataset "small_uniform" "uniform" 1000 "--avg-degree 10"
generate_dataset "small_social" "power-law" 1000 "--avg-degree 15"
generate_dataset "small_grid" "grid" 900 "--size 30"  # 30x30 grid
generate_dataset "small_tree" "tree" 1000 "--depth 5 --branching 3"

# Medium datasets (standard benchmarks)
echo -e "${BLUE}--- Medium Datasets (Standard Benchmarks) ---${NC}"
generate_dataset "medium_uniform" "uniform" 10000 "--avg-degree 10"
generate_dataset "medium_social" "power-law" 10000 "--avg-degree 20"
generate_dataset "medium_grid" "grid" 10000 "--size 100"  # 100x100 grid
generate_dataset "medium_tree" "tree" 10000 "--depth 6 --branching 3"

# Large datasets (scalability tests)
echo -e "${BLUE}--- Large Datasets (Scalability Tests) ---${NC}"
generate_dataset "large_uniform" "uniform" 100000 "--avg-degree 10"
generate_dataset "large_social" "power-law" 100000 "--avg-degree 25"

# XLarge datasets (stress tests) - Optional, commented out by default
# echo -e "${BLUE}--- XLarge Datasets (Stress Tests) ---${NC}"
# generate_dataset "xlarge_uniform" "uniform" 1000000 "--avg-degree 10"
# generate_dataset "xlarge_social" "power-law" 1000000 "--avg-degree 30"

# Summary
echo -e "${GREEN}=== Data Generation Complete ===${NC}"
echo ""
echo "Generated datasets in: $DATA_DIR"
echo ""
echo "Dataset sizes:"
du -sh "$DATA_DIR"/* | awk '{print "  " $2 ": " $1}'
echo ""
echo -e "${GREEN}✓ All test data ready!${NC}"
