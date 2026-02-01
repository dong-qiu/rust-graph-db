/// Rust Graph Database
///
/// A high-performance graph database implementation in Rust, compatible with openGauss-graph.
///
/// # Architecture
///
/// ```text
/// ┌──────────────────────────────────────────────────┐
/// │           Rust Graph Database                    │
/// ├──────────────────────────────────────────────────┤
/// │  ┌────────────────────────────────┐              │
/// │  │   Cypher Parser (pest)         │              │
/// │  └────────────┬───────────────────┘              │
/// │               ↓                                   │
/// │  ┌────────────────────────────────┐              │
/// │  │   Query Planner                │              │
/// │  └────────────┬───────────────────┘              │
/// │               ↓                                   │
/// │  ┌────────────────────────────────┐              │
/// │  │   Graph Executor               │              │
/// │  └────────────┬───────────────────┘              │
/// │               ↓                                   │
/// │  ┌────────────────────────────────┐              │
/// │  │   Storage Engine (RocksDB)     │              │
/// │  └────────────────────────────────┘              │
/// └──────────────────────────────────────────────────┘
/// ```
///
/// # Modules
///
/// - `types`: Core data types (Graphid, Vertex, Edge, GraphPath)
/// - `jsonb`: JSONB compatibility layer for openGauss-graph
/// - `storage`: Storage engine abstraction and RocksDB implementation
/// - `parser`: Cypher query parser
/// - `executor`: Query execution engine
/// - `algorithms`: Graph algorithms (shortest path, etc.)
/// - `tools`: Utilities (data import, etc.)

pub mod types;
pub mod jsonb;
pub mod storage;
pub mod parser;
pub mod executor;
pub mod algorithms;
pub mod tools;

// Re-export commonly used types
pub use types::{Edge, Graphid, GraphPath, Vertex};

// Re-export storage types
pub use storage::{GraphStorage, GraphTransaction, SharedStorage, StorageError, StorageResult};

// Re-export parser types
pub use parser::{parse_cypher, ParseError, ParseResult};
pub use parser::ast::{CypherQuery, Expression, Pattern, NodePattern, EdgePattern};

// Re-export executor types
pub use executor::{ExecutionError, ExecutionResult, QueryExecutor, Row, Value};

// Re-export algorithm types
pub use algorithms::{dijkstra, shortest_path, variable_length_expand, AlgorithmError, AlgorithmResult, ShortestPathResult, VariableLengthPath, VleOptions};

// Re-export tool types
pub use tools::{ExportFormat, ExportOptions, ImportOptions, ImportStats, ToolError, ToolResult, export_to_csv, export_to_json, import_from_csv, import_from_json};

/// Result type used throughout the library
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
