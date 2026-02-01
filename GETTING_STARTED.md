# Getting Started with Rust Graph Database

Welcome to the Rust Graph Database project! This guide will help you get started with using and developing the database.

## Quick Start

### Prerequisites

- **Rust**: 1.70 or later (install from [rustup.rs](https://rustup.rs/))
- **Git**: For cloning the repository

### Installation

```bash
# Clone the repository
cd /path/to/openGauss-graph/rust-graph-db

# Build the project
cargo build

# Run tests
cargo test

# Run the example
cargo run --example basic_usage
```

### Build Output

```
✅ Finished `dev` profile in 3.74s
✅ 32 tests passing
```

## Basic Usage

### 1. Creating Vertices (Nodes)

```rust
use rust_graph_db::{Graphid, Vertex};
use serde_json::json;

// Create a unique identifier
let id = Graphid::new(1, 100).unwrap();

// Create a vertex with properties
let vertex = Vertex::new(id, "Person", json!({
    "name": "Alice",
    "age": 30,
    "city": "Beijing"
}));

// Access properties
println!("Name: {}", vertex.get_property("name").unwrap());
```

### 2. Creating Edges (Relationships)

```rust
use rust_graph_db::{Edge, Graphid};
use serde_json::json;

let start = Graphid::new(1, 100).unwrap();
let end = Graphid::new(1, 101).unwrap();
let edge_id = Graphid::new(2, 200).unwrap();

let edge = Edge::new(edge_id, start, end, "KNOWS", json!({
    "since": 2020,
    "relationship": "friend"
}));

println!("Edge: {} -> {}", edge.start, edge.end);
```

### 3. Creating Paths

```rust
use rust_graph_db::GraphPath;

let path = GraphPath::from_parts(
    vec![vertex1, vertex2, vertex3],
    vec![edge1, edge2]
).unwrap();

println!("Path length: {} edges", path.len());
println!("Start: {}", path.start().unwrap().id);
println!("End: {}", path.end().unwrap().id);
```

### 4. Working with Properties

```rust
// Set properties
vertex.set_property("name", json!("Bob"));
vertex.set_property("age", json!(25));

// Get properties
if let Some(name) = vertex.get_property("name") {
    println!("Name: {}", name);
}

// Remove properties
vertex.remove_property("age");

// Check existence
if vertex.has_property("name") {
    println!("Vertex has a name");
}

// Get all property keys
let keys = vertex.property_keys();
println!("Properties: {:?}", keys);
```

## Core Concepts

### Graphid

A 64-bit unique identifier that combines:
- **Label ID** (16 bits): Type/class identifier
- **Local ID** (48 bits): Unique within the label

```rust
let id = Graphid::new(labid, locid)?;

println!("Label: {}", id.labid());   // Extract label ID
println!("Local: {}", id.locid());   // Extract local ID
println!("Display: {}", id);         // Format: "labid.locid"
```

**Constraints:**
- Label ID: 0 to 65,535 (2^16 - 1)
- Local ID: 0 to 281,474,976,710,655 (2^48 - 1)

### Vertex

Represents a graph node with:
- Unique ID (Graphid)
- Label (string)
- Properties (JSON object)

```rust
pub struct Vertex {
    pub id: Graphid,
    pub label: String,
    pub properties: JsonValue,
}
```

### Edge

Represents a directed relationship with:
- Unique ID (Graphid)
- Start vertex ID
- End vertex ID
- Label (string)
- Properties (JSON object)

```rust
pub struct Edge {
    pub id: Graphid,
    pub start: Graphid,
    pub end: Graphid,
    pub label: String,
    pub properties: JsonValue,
}
```

**Special operations:**
```rust
// Check if edge is a self-loop
if edge.is_self_loop() {
    println!("Self-loop detected!");
}

// Reverse edge direction
let reversed = edge.reverse();
```

### GraphPath

Represents a path through the graph:
- Sequence of vertices
- Sequence of edges connecting them

```rust
pub struct GraphPath {
    pub vertices: Vec<Vertex>,
    pub edges: Vec<Edge>,
}
```

**Invariants:**
- `vertices.len() == edges.len() + 1`
- Each edge connects consecutive vertices
- Paths are validated on creation

## Testing

### Run All Tests

```bash
cargo test
```

### Run Specific Test Module

```bash
cargo test types::graphid
cargo test types::vertex
cargo test jsonb
```

### Run with Output

```bash
cargo test -- --nocapture
```

### Test Coverage

Currently: **32 tests, 100% passing**

- Graphid: 8 tests
- Vertex: 7 tests
- Edge: 8 tests
- GraphPath: 9 tests
- JSONB: 6 tests

## Examples

### Run the Basic Usage Example

```bash
cargo run --example basic_usage
```

This demonstrates:
1. Creating vertices and edges
2. Building graph paths
3. Property manipulation
4. Graphid operations
5. Edge operations
6. Path validation
7. JSON serialization

## Development

### Project Structure

```
rust-graph-db/
├── src/
│   ├── lib.rs           # Library entry point
│   ├── types/           # Core data types
│   │   ├── mod.rs
│   │   ├── graphid.rs   # Graphid implementation
│   │   ├── vertex.rs    # Vertex implementation
│   │   ├── edge.rs      # Edge implementation
│   │   └── path.rs      # GraphPath implementation
│   ├── jsonb/           # JSONB compatibility
│   │   └── mod.rs
│   ├── storage/         # Storage engine (TODO)
│   ├── parser/          # Cypher parser (TODO)
│   ├── executor/        # Query executor (TODO)
│   └── algorithms/      # Graph algorithms (TODO)
├── tests/               # Integration tests
├── benches/             # Performance benchmarks
├── examples/            # Usage examples
└── Cargo.toml           # Dependencies
```

### Code Quality

Before submitting code:

```bash
# Format code
cargo fmt

# Check for warnings
cargo clippy

# Run tests
cargo test

# Build in release mode
cargo build --release
```

### Adding New Features

1. Write tests first
2. Implement the feature
3. Ensure all tests pass
4. Run clippy for warnings
5. Update documentation
6. Add example usage if applicable

## Performance

### Benchmarks

```bash
cargo bench
```

This runs criterion benchmarks for:
- Graphid creation
- Vertex creation
- Edge creation
- Property access

### Current Performance

On Apple M-series (example results):
- Graphid creation: ~5 ns
- Vertex creation: ~100 ns
- Edge creation: ~120 ns
- Property access: ~10 ns

## Compatibility with openGauss-graph

This implementation is designed to be compatible with openGauss-graph:

### Data Format Compatibility

1. **Graphid Format**: Identical 64-bit layout
   - High 16 bits: Label ID
   - Low 48 bits: Local ID

2. **JSONB Properties**: Compatible JSON storage
   - Phase 1: UTF-8 JSON strings (current)
   - Phase 2: Binary JSONB format (planned)

3. **Type System**: Same core types
   - Vertex, Edge, GraphPath
   - Property-based storage

### Migration Path

Future phases will include:
- Data import tool from openGauss-graph
- Full JSONB binary compatibility
- Cypher query language support

## Next Steps

### Immediate Next Steps (You)

1. Explore the example: `cargo run --example basic_usage`
2. Read the API documentation: `cargo doc --open`
3. Try creating your own vertices and edges
4. Run the test suite: `cargo test`

### Development Roadmap

See [IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md) for detailed progress.

**Phase 1 (Complete)**: Core data types ✅
**Phase 2 (Next)**: Storage engine with RocksDB
**Phase 3**: Cypher parser
**Phase 4**: Query executor
**Phase 5**: Graph algorithms
**Phase 6**: Integration and testing

## Troubleshooting

### Rust Not Found

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### Build Errors

```bash
# Clean and rebuild
cargo clean
cargo build
```

### Test Failures

```bash
# Run tests with output
cargo test -- --nocapture --test-threads=1
```

## Resources

### Documentation

- README.md: Project overview
- IMPLEMENTATION_STATUS.md: Detailed progress tracking
- This file: Getting started guide

### Code

- `examples/basic_usage.rs`: Comprehensive example
- `src/types/`: Core type implementations
- Tests in each module for usage examples

### External Resources

- [Rust Book](https://doc.rust-lang.org/book/)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/)
- [openGauss-graph Repository](https://gitee.com/opengauss/openGauss-graph)

## Getting Help

### Questions?

1. Check the examples in `examples/`
2. Read the API documentation: `cargo doc --open`
3. Look at the test files for usage patterns
4. Review [IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md)

### Found a Bug?

1. Check if it's a known issue
2. Create a minimal reproduction
3. Run tests to verify: `cargo test`
4. Report with details

## Contributing

Contributions are welcome! Please:

1. Write tests for new features
2. Follow Rust conventions
3. Run `cargo fmt` and `cargo clippy`
4. Update documentation
5. Add examples for significant features

---

**Happy coding!** 🦀

For more information, see:
- [README.md](README.md) - Project overview
- [IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md) - Detailed status

**Last Updated**: 2026-01-30
