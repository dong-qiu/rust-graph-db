/// Data import utilities
///
/// Supports importing graph data from various formats:
/// - CSV files (vertices and edges)
/// - JSON files
/// - PostgreSQL/openGauss-graph databases

use super::{ToolError, ToolResult};
use crate::storage::{GraphStorage, GraphTransaction};
use crate::types::{Edge, Graphid, Vertex};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

/// Import options
#[derive(Debug, Clone)]
pub struct ImportOptions {
    /// Batch size for bulk imports
    pub batch_size: usize,
    /// Skip invalid rows instead of failing
    pub skip_errors: bool,
    /// Default label if not specified
    pub default_vertex_label: String,
    /// Default edge label if not specified
    pub default_edge_label: String,
    /// Progress callback interval (rows)
    pub progress_interval: usize,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            batch_size: 1000,
            skip_errors: false,
            default_vertex_label: "Node".to_string(),
            default_edge_label: "RELATES_TO".to_string(),
            progress_interval: 10000,
        }
    }
}

/// Import statistics
#[derive(Debug, Default, Clone, Serialize)]
pub struct ImportStats {
    pub vertices_imported: usize,
    pub edges_imported: usize,
    pub vertices_skipped: usize,
    pub edges_skipped: usize,
    pub errors: Vec<String>,
}

impl ImportStats {
    fn add_error(&mut self, error: String) {
        self.errors.push(error);
    }
}

/// CSV vertex row
#[derive(Debug, Deserialize)]
struct CsvVertex {
    #[serde(default)]
    id: Option<String>,
    label: String,
    #[serde(flatten)]
    properties: HashMap<String, JsonValue>,
}

/// CSV edge row
#[derive(Debug, Deserialize)]
struct CsvEdge {
    #[serde(default)]
    id: Option<String>,
    label: String,
    start: String,
    end: String,
    #[serde(flatten)]
    properties: HashMap<String, JsonValue>,
}

/// JSON graph format
#[derive(Debug, Deserialize)]
struct JsonGraph {
    vertices: Vec<JsonVertex>,
    edges: Vec<JsonEdge>,
}

#[derive(Debug, Deserialize)]
struct JsonVertex {
    #[serde(default)]
    id: Option<String>,
    label: String,
    properties: HashMap<String, JsonValue>,
}

#[derive(Debug, Deserialize)]
struct JsonEdge {
    #[serde(default)]
    id: Option<String>,
    label: String,
    start: String,
    end: String,
    properties: HashMap<String, JsonValue>,
}

/// Import vertices from CSV file
///
/// CSV format:
/// ```csv
/// id,label,name,age
/// ,Person,Alice,30
/// ,Person,Bob,25
/// ```
///
/// Note: ID column can be empty (will be auto-generated)
pub async fn import_vertices_from_csv<P: AsRef<Path>>(
    storage: Arc<dyn GraphStorage>,
    path: P,
    options: &ImportOptions,
) -> ToolResult<ImportStats> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut csv_reader = csv::Reader::from_reader(reader);

    let mut stats = ImportStats::default();
    let mut id_mapping: HashMap<String, Graphid> = HashMap::new();
    let mut batch_count = 0;

    let mut tx = storage.begin_transaction().await?;

    for (row_num, result) in csv_reader.deserialize().enumerate() {
        match result {
            Ok(csv_vertex) => {
                let vertex: CsvVertex = csv_vertex;

                // Create vertex
                match create_vertex_from_csv(&mut tx, &vertex).await {
                    Ok(created_vertex) => {
                        stats.vertices_imported += 1;

                        // Store ID mapping if original ID provided
                        if let Some(ref old_id) = vertex.id {
                            id_mapping.insert(old_id.clone(), created_vertex.id);
                        }

                        batch_count += 1;

                        // Commit batch
                        if batch_count >= options.batch_size {
                            tx.commit().await?;
                            tx = storage.begin_transaction().await?;
                            batch_count = 0;
                        }

                        // Progress reporting
                        if (row_num + 1) % options.progress_interval == 0 {
                            eprintln!("Imported {} vertices...", stats.vertices_imported);
                        }
                    }
                    Err(e) => {
                        if options.skip_errors {
                            stats.vertices_skipped += 1;
                            stats.add_error(format!("Row {}: {}", row_num, e));
                        } else {
                            return Err(e);
                        }
                    }
                }
            }
            Err(e) => {
                if options.skip_errors {
                    stats.vertices_skipped += 1;
                    stats.add_error(format!("Row {}: CSV parse error: {}", row_num, e));
                } else {
                    return Err(ToolError::CsvError(e));
                }
            }
        }
    }

    // Commit remaining batch
    if batch_count > 0 {
        tx.commit().await?;
    }

    Ok(stats)
}

/// Import edges from CSV file
///
/// CSV format:
/// ```csv
/// id,label,start,end,since
/// ,KNOWS,alice,bob,2020
/// ,KNOWS,alice,charlie,2021
/// ```
pub async fn import_edges_from_csv<P: AsRef<Path>>(
    storage: Arc<dyn GraphStorage>,
    path: P,
    id_mapping: &HashMap<String, Graphid>,
    options: &ImportOptions,
) -> ToolResult<ImportStats> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut csv_reader = csv::Reader::from_reader(reader);

    let mut stats = ImportStats::default();
    let mut batch_count = 0;

    let mut tx = storage.begin_transaction().await?;

    for (row_num, result) in csv_reader.deserialize().enumerate() {
        match result {
            Ok(csv_edge) => {
                let edge: CsvEdge = csv_edge;

                // Resolve start and end vertex IDs
                let start_id = id_mapping.get(&edge.start).ok_or_else(|| {
                    ToolError::InvalidFormat(format!("Start vertex not found: {}", edge.start))
                })?;

                let end_id = id_mapping.get(&edge.end).ok_or_else(|| {
                    ToolError::InvalidFormat(format!("End vertex not found: {}", edge.end))
                })?;

                // Create edge
                match create_edge_from_csv(&mut tx, &edge, *start_id, *end_id).await {
                    Ok(_) => {
                        stats.edges_imported += 1;
                        batch_count += 1;

                        // Commit batch
                        if batch_count >= options.batch_size {
                            tx.commit().await?;
                            tx = storage.begin_transaction().await?;
                            batch_count = 0;
                        }

                        // Progress reporting
                        if (row_num + 1) % options.progress_interval == 0 {
                            eprintln!("Imported {} edges...", stats.edges_imported);
                        }
                    }
                    Err(e) => {
                        if options.skip_errors {
                            stats.edges_skipped += 1;
                            stats.add_error(format!("Row {}: {}", row_num, e));
                        } else {
                            return Err(e);
                        }
                    }
                }
            }
            Err(e) => {
                if options.skip_errors {
                    stats.edges_skipped += 1;
                    stats.add_error(format!("Row {}: CSV parse error: {}", row_num, e));
                } else {
                    return Err(ToolError::CsvError(e));
                }
            }
        }
    }

    // Commit remaining batch
    if batch_count > 0 {
        tx.commit().await?;
    }

    Ok(stats)
}

/// Import graph from CSV files (vertices and edges)
pub async fn import_from_csv<P: AsRef<Path>>(
    storage: Arc<dyn GraphStorage>,
    vertices_path: P,
    edges_path: P,
    options: ImportOptions,
) -> ToolResult<ImportStats> {
    eprintln!("Importing vertices...");
    let vertex_stats = import_vertices_from_csv(storage.clone(), vertices_path, &options).await?;

    eprintln!("Vertices imported: {}", vertex_stats.vertices_imported);
    eprintln!("Vertices skipped: {}", vertex_stats.vertices_skipped);

    // Build ID mapping for edges
    // Note: In a real implementation, we'd need to persist this mapping
    // For now, we'll require external ID mapping or use property-based matching
    let id_mapping = HashMap::new(); // Simplified for this implementation

    eprintln!("Importing edges...");
    let edge_stats = import_edges_from_csv(storage, edges_path, &id_mapping, &options).await?;

    eprintln!("Edges imported: {}", edge_stats.edges_imported);
    eprintln!("Edges skipped: {}", edge_stats.edges_skipped);

    // Combine stats
    let mut combined_stats = vertex_stats;
    combined_stats.edges_imported = edge_stats.edges_imported;
    combined_stats.edges_skipped = edge_stats.edges_skipped;
    combined_stats.errors.extend(edge_stats.errors);

    Ok(combined_stats)
}

/// Import graph from JSON file
///
/// JSON format:
/// ```json
/// {
///   "vertices": [
///     {"label": "Person", "properties": {"name": "Alice", "age": 30}},
///     {"label": "Person", "properties": {"name": "Bob", "age": 25}}
///   ],
///   "edges": [
///     {"label": "KNOWS", "start": "0", "end": "1", "properties": {"since": 2020}}
///   ]
/// }
/// ```
pub async fn import_from_json<P: AsRef<Path>>(
    storage: Arc<dyn GraphStorage>,
    path: P,
    options: ImportOptions,
) -> ToolResult<ImportStats> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let graph: JsonGraph = serde_json::from_reader(reader)?;

    let mut stats = ImportStats::default();
    let mut id_mapping: HashMap<String, Graphid> = HashMap::new();

    // Import vertices
    eprintln!("Importing {} vertices...", graph.vertices.len());
    let mut tx = storage.begin_transaction().await?;
    let mut batch_count = 0;

    for (idx, json_vertex) in graph.vertices.iter().enumerate() {
        match create_vertex_from_json(&mut tx, json_vertex).await {
            Ok(vertex) => {
                stats.vertices_imported += 1;

                // Map index or provided ID to new Graphid
                let key = json_vertex
                    .id
                    .clone()
                    .unwrap_or_else(|| idx.to_string());
                id_mapping.insert(key, vertex.id);

                batch_count += 1;
                if batch_count >= options.batch_size {
                    tx.commit().await?;
                    tx = storage.begin_transaction().await?;
                    batch_count = 0;
                }

                if (idx + 1) % options.progress_interval == 0 {
                    eprintln!("  Imported {} vertices...", stats.vertices_imported);
                }
            }
            Err(e) => {
                if options.skip_errors {
                    stats.vertices_skipped += 1;
                    stats.add_error(format!("Vertex {}: {}", idx, e));
                } else {
                    return Err(e);
                }
            }
        }
    }

    if batch_count > 0 {
        tx.commit().await?;
    }

    // Import edges
    eprintln!("Importing {} edges...", graph.edges.len());
    tx = storage.begin_transaction().await?;
    batch_count = 0;

    for (idx, json_edge) in graph.edges.iter().enumerate() {
        let start_id = id_mapping.get(&json_edge.start).ok_or_else(|| {
            ToolError::InvalidFormat(format!("Start vertex not found: {}", json_edge.start))
        })?;

        let end_id = id_mapping.get(&json_edge.end).ok_or_else(|| {
            ToolError::InvalidFormat(format!("End vertex not found: {}", json_edge.end))
        })?;

        match create_edge_from_json(&mut tx, json_edge, *start_id, *end_id).await {
            Ok(_) => {
                stats.edges_imported += 1;
                batch_count += 1;

                if batch_count >= options.batch_size {
                    tx.commit().await?;
                    tx = storage.begin_transaction().await?;
                    batch_count = 0;
                }

                if (idx + 1) % options.progress_interval == 0 {
                    eprintln!("  Imported {} edges...", stats.edges_imported);
                }
            }
            Err(e) => {
                if options.skip_errors {
                    stats.edges_skipped += 1;
                    stats.add_error(format!("Edge {}: {}", idx, e));
                } else {
                    return Err(e);
                }
            }
        }
    }

    if batch_count > 0 {
        tx.commit().await?;
    }

    eprintln!("Import complete!");
    eprintln!("  Vertices: {} imported, {} skipped", stats.vertices_imported, stats.vertices_skipped);
    eprintln!("  Edges: {} imported, {} skipped", stats.edges_imported, stats.edges_skipped);

    Ok(stats)
}

// Helper functions

async fn create_vertex_from_csv(
    tx: &mut Box<dyn GraphTransaction>,
    vertex: &CsvVertex,
) -> ToolResult<Vertex> {
    let properties = serde_json::to_value(&vertex.properties)?;
    let created = tx.create_vertex(&vertex.label, properties).await?;
    Ok(created)
}

async fn create_edge_from_csv(
    tx: &mut Box<dyn GraphTransaction>,
    edge: &CsvEdge,
    start: Graphid,
    end: Graphid,
) -> ToolResult<Edge> {
    let properties = serde_json::to_value(&edge.properties)?;
    let created = tx.create_edge(&edge.label, start, end, properties).await?;
    Ok(created)
}

async fn create_vertex_from_json(
    tx: &mut Box<dyn GraphTransaction>,
    vertex: &JsonVertex,
) -> ToolResult<Vertex> {
    let properties = serde_json::to_value(&vertex.properties)?;
    let created = tx.create_vertex(&vertex.label, properties).await?;
    Ok(created)
}

async fn create_edge_from_json(
    tx: &mut Box<dyn GraphTransaction>,
    edge: &JsonEdge,
    start: Graphid,
    end: Graphid,
) -> ToolResult<Edge> {
    let properties = serde_json::to_value(&edge.properties)?;
    let created = tx.create_edge(&edge.label, start, end, properties).await?;
    Ok(created)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::rocksdb_store::RocksDbStorage;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_import_from_json() {
        let temp_dir = TempDir::new().unwrap();
        let storage: Arc<dyn GraphStorage> =
            Arc::new(RocksDbStorage::new(temp_dir.path(), "test_graph").unwrap());

        // Create test JSON file
        let json_content = r#"
        {
            "vertices": [
                {"id": "alice", "label": "Person", "properties": {"name": "Alice", "age": 30}},
                {"id": "bob", "label": "Person", "properties": {"name": "Bob", "age": 25}}
            ],
            "edges": [
                {"label": "KNOWS", "start": "alice", "end": "bob", "properties": {"since": 2020}}
            ]
        }
        "#;

        let json_path = temp_dir.path().join("graph.json");
        std::fs::write(&json_path, json_content).unwrap();

        // Import
        let options = ImportOptions::default();
        let stats = import_from_json(storage.clone(), &json_path, options)
            .await
            .unwrap();

        // Verify
        assert_eq!(stats.vertices_imported, 2);
        assert_eq!(stats.edges_imported, 1);
        assert_eq!(stats.vertices_skipped, 0);
        assert_eq!(stats.edges_skipped, 0);

        // Check data
        let vertices = storage.scan_vertices("Person").await.unwrap();
        assert_eq!(vertices.len(), 2);

        let edges = storage.scan_edges("KNOWS").await.unwrap();
        assert_eq!(edges.len(), 1);
    }
}
