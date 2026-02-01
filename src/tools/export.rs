/// Data export utilities
///
/// Supports exporting graph data to various formats:
/// - CSV files (vertices and edges)
/// - JSON files
/// - GraphML (future)

use super::ToolResult;
use crate::storage::GraphStorage;
use serde::Serialize;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

/// Export format
#[derive(Debug, Clone, Copy)]
pub enum ExportFormat {
    Csv,
    Json,
    GraphMl,
}

/// Export options
#[derive(Debug, Clone)]
pub struct ExportOptions {
    /// Pretty-print JSON output
    pub pretty_json: bool,
    /// Include header row in CSV
    pub csv_header: bool,
    /// Progress reporting interval
    pub progress_interval: usize,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            pretty_json: true,
            csv_header: true,
            progress_interval: 10000,
        }
    }
}

/// Export vertices to CSV file
pub async fn export_vertices_to_csv<P: AsRef<Path>>(
    storage: Arc<dyn GraphStorage>,
    path: P,
    label: &str,
    options: &ExportOptions,
) -> ToolResult<usize> {
    let vertices = storage.scan_vertices(label).await?;

    let file = File::create(path)?;
    let mut writer = csv::Writer::from_writer(file);

    let mut count = 0;

    for vertex in vertices {
        // Convert vertex to CSV-friendly format
        let mut record = vec![
            format!("{:?}", vertex.id),
            vertex.label.clone(),
        ];

        // Add properties as JSON string
        let props_json = serde_json::to_string(&vertex.properties)?;
        record.push(props_json);

        writer.write_record(&record)?;
        count += 1;

        if count % options.progress_interval == 0 {
            eprintln!("Exported {} vertices...", count);
        }
    }

    writer.flush()?;
    Ok(count)
}

/// Export edges to CSV file
pub async fn export_edges_to_csv<P: AsRef<Path>>(
    storage: Arc<dyn GraphStorage>,
    path: P,
    label: &str,
    options: &ExportOptions,
) -> ToolResult<usize> {
    let edges = storage.scan_edges(label).await?;

    let file = File::create(path)?;
    let mut writer = csv::Writer::from_writer(file);

    let mut count = 0;

    for edge in edges {
        let record = vec![
            format!("{:?}", edge.id),
            edge.label.clone(),
            format!("{:?}", edge.start),
            format!("{:?}", edge.end),
            serde_json::to_string(&edge.properties)?,
        ];

        writer.write_record(&record)?;
        count += 1;

        if count % options.progress_interval == 0 {
            eprintln!("Exported {} edges...", count);
        }
    }

    writer.flush()?;
    Ok(count)
}

/// Export entire graph to CSV files (separate files for vertices and edges)
pub async fn export_to_csv<P: AsRef<Path>>(
    storage: Arc<dyn GraphStorage>,
    vertices_path: P,
    edges_path: P,
    vertex_labels: Vec<String>,
    edge_labels: Vec<String>,
    options: ExportOptions,
) -> ToolResult<(usize, usize)> {
    let mut total_vertices = 0;
    let mut total_edges = 0;

    // Export vertices
    eprintln!("Exporting vertices...");
    for label in vertex_labels {
        let count = export_vertices_to_csv(
            storage.clone(),
            &vertices_path,
            &label,
            &options,
        )
        .await?;
        total_vertices += count;
        eprintln!("  {}: {} vertices", label, count);
    }

    // Export edges
    eprintln!("Exporting edges...");
    for label in edge_labels {
        let count = export_edges_to_csv(
            storage.clone(),
            &edges_path,
            &label,
            &options,
        )
        .await?;
        total_edges += count;
        eprintln!("  {}: {} edges", label, count);
    }

    Ok((total_vertices, total_edges))
}

/// JSON export format
#[derive(Debug, Serialize)]
struct JsonGraphExport {
    vertices: Vec<JsonVertexExport>,
    edges: Vec<JsonEdgeExport>,
}

#[derive(Debug, Clone, Serialize)]
struct JsonVertexExport {
    id: String,
    label: String,
    properties: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
struct JsonEdgeExport {
    id: String,
    label: String,
    start: String,
    end: String,
    properties: serde_json::Value,
}

/// Export entire graph to JSON file
pub async fn export_to_json<P: AsRef<Path>>(
    storage: Arc<dyn GraphStorage>,
    path: P,
    vertex_labels: Vec<String>,
    edge_labels: Vec<String>,
    options: ExportOptions,
) -> ToolResult<(usize, usize)> {
    let mut all_vertices = Vec::new();
    let mut all_edges = Vec::new();

    // Collect vertices
    eprintln!("Collecting vertices...");
    for label in vertex_labels {
        let vertices = storage.scan_vertices(&label).await?;
        eprintln!("  {}: {} vertices", label, vertices.len());

        for vertex in vertices {
            all_vertices.push(JsonVertexExport {
                id: format!("{:?}", vertex.id),
                label: vertex.label,
                properties: vertex.properties,
            });
        }
    }

    // Collect edges
    eprintln!("Collecting edges...");
    for label in edge_labels {
        let edges = storage.scan_edges(&label).await?;
        eprintln!("  {}: {} edges", label, edges.len());

        for edge in edges {
            all_edges.push(JsonEdgeExport {
                id: format!("{:?}", edge.id),
                label: edge.label,
                start: format!("{:?}", edge.start),
                end: format!("{:?}", edge.end),
                properties: edge.properties,
            });
        }
    }

    // Write JSON
    let graph_export = JsonGraphExport {
        vertices: all_vertices.clone(),
        edges: all_edges.clone(),
    };

    let file = File::create(path)?;
    if options.pretty_json {
        serde_json::to_writer_pretty(file, &graph_export)?;
    } else {
        serde_json::to_writer(file, &graph_export)?;
    }

    eprintln!("Export complete!");
    eprintln!("  Total vertices: {}", all_vertices.len());
    eprintln!("  Total edges: {}", all_edges.len());

    Ok((all_vertices.len(), all_edges.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::rocksdb_store::RocksDbStorage;
    use serde_json::json;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_export_to_json() {
        let temp_dir = TempDir::new().unwrap();
        let storage: Arc<dyn GraphStorage> =
            Arc::new(RocksDbStorage::new(temp_dir.path(), "test_graph").unwrap());

        // Create test data
        let mut tx = storage.begin_transaction().await.unwrap();

        let alice = tx
            .create_vertex("Person", json!({"name": "Alice", "age": 30}))
            .await
            .unwrap();

        let bob = tx
            .create_vertex("Person", json!({"name": "Bob", "age": 25}))
            .await
            .unwrap();

        tx.create_edge("KNOWS", alice.id, bob.id, json!({"since": 2020}))
            .await
            .unwrap();

        tx.commit().await.unwrap();

        // Export
        let json_path = temp_dir.path().join("export.json");
        let options = ExportOptions::default();

        let (v_count, e_count) = export_to_json(
            storage,
            &json_path,
            vec!["Person".to_string()],
            vec!["KNOWS".to_string()],
            options,
        )
        .await
        .unwrap();

        // Verify
        assert_eq!(v_count, 2);
        assert_eq!(e_count, 1);

        // Check file exists and is valid JSON
        let content = std::fs::read_to_string(&json_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(parsed["vertices"].as_array().unwrap().len(), 2);
        assert_eq!(parsed["edges"].as_array().unwrap().len(), 1);
    }
}
