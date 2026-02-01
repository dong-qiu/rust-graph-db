/// Import/Export demonstration
///
/// This example shows how to:
/// - Import graph data from JSON files
/// - Export graph data to JSON files
/// - Use import/export options

use rust_graph_db::storage::rocksdb_store::RocksDbStorage;
use rust_graph_db::storage::GraphStorage;
use rust_graph_db::{import_from_json, export_to_json, ImportOptions, ExportOptions};
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Import/Export Demo ===\n");

    // Setup storage
    let temp_dir = TempDir::new()?;
    let storage = Arc::new(RocksDbStorage::new(temp_dir.path(), "demo_graph")?);

    // Example 1: Create sample JSON file
    println!("1. Creating sample JSON file...");
    let json_content = r#"
    {
        "vertices": [
            {"id": "alice", "label": "Person", "properties": {"name": "Alice", "age": 30, "city": "Beijing"}},
            {"id": "bob", "label": "Person", "properties": {"name": "Bob", "age": 25, "city": "Shanghai"}},
            {"id": "charlie", "label": "Person", "properties": {"name": "Charlie", "age": 35, "city": "Shenzhen"}},
            {"id": "rust", "label": "Language", "properties": {"name": "Rust", "year": 2010}},
            {"id": "python", "label": "Language", "properties": {"name": "Python", "year": 1991}}
        ],
        "edges": [
            {"label": "KNOWS", "start": "alice", "end": "bob", "properties": {"since": 2020}},
            {"label": "KNOWS", "start": "alice", "end": "charlie", "properties": {"since": 2018}},
            {"label": "KNOWS", "start": "bob", "end": "charlie", "properties": {"since": 2019}},
            {"label": "USES", "start": "alice", "end": "rust", "properties": {"level": "expert"}},
            {"label": "USES", "start": "bob", "end": "python", "properties": {"level": "intermediate"}},
            {"label": "USES", "start": "charlie", "end": "rust", "properties": {"level": "beginner"}}
        ]
    }
    "#;

    let import_path = temp_dir.path().join("sample_graph.json");
    std::fs::write(&import_path, json_content)?;
    println!("   Created: {:?}\n", import_path);

    // Example 2: Import from JSON
    println!("2. Importing graph from JSON...");
    let import_options = ImportOptions {
        batch_size: 100,
        skip_errors: false,
        progress_interval: 2,
        ..Default::default()
    };

    let stats = import_from_json(storage.clone(), &import_path, import_options).await?;

    println!("\n   Import Statistics:");
    println!("   - Vertices imported: {}", stats.vertices_imported);
    println!("   - Edges imported: {}", stats.edges_imported);
    println!("   - Vertices skipped: {}", stats.vertices_skipped);
    println!("   - Edges skipped: {}", stats.edges_skipped);
    println!("   - Errors: {}\n", stats.errors.len());

    // Example 3: Verify imported data
    println!("3. Verifying imported data...");
    let persons = storage.scan_vertices("Person").await?;
    let languages = storage.scan_vertices("Language").await?;
    let knows_edges = storage.scan_edges("KNOWS").await?;
    let uses_edges = storage.scan_edges("USES").await?;

    println!("   - Person vertices: {}", persons.len());
    println!("   - Language vertices: {}", languages.len());
    println!("   - KNOWS edges: {}", knows_edges.len());
    println!("   - USES edges: {}\n", uses_edges.len());

    // Example 4: Export to JSON
    println!("4. Exporting graph to JSON...");
    let export_path = temp_dir.path().join("exported_graph.json");
    let export_options = ExportOptions {
        pretty_json: true,
        csv_header: true,
        progress_interval: 2,
    };

    let (v_count, e_count) = export_to_json(
        storage.clone(),
        &export_path,
        vec!["Person".to_string(), "Language".to_string()],
        vec!["KNOWS".to_string(), "USES".to_string()],
        export_options,
    )
    .await?;

    println!("\n   Export Statistics:");
    println!("   - Vertices exported: {}", v_count);
    println!("   - Edges exported: {}", e_count);
    println!("   - Output file: {:?}\n", export_path);

    // Example 5: Verify exported file
    println!("5. Verifying exported file...");
    let exported_content = std::fs::read_to_string(&export_path)?;
    let exported_json: serde_json::Value = serde_json::from_str(&exported_content)?;

    println!("   - Vertices in file: {}", exported_json["vertices"].as_array().unwrap().len());
    println!("   - Edges in file: {}\n", exported_json["edges"].as_array().unwrap().len());

    // Example 6: Show sample of exported data
    println!("6. Sample exported data:");
    if let Some(first_vertex) = exported_json["vertices"].as_array().unwrap().first() {
        println!("   First vertex:");
        println!("   {}\n", serde_json::to_string_pretty(first_vertex)?);
    }

    if let Some(first_edge) = exported_json["edges"].as_array().unwrap().first() {
        println!("   First edge:");
        println!("   {}\n", serde_json::to_string_pretty(first_edge)?);
    }

    // Example 7: Re-import to verify round-trip
    println!("7. Testing round-trip import...");
    let temp_dir2 = TempDir::new()?;
    let storage2 = Arc::new(RocksDbStorage::new(temp_dir2.path(), "demo_graph2")?);

    let reimport_stats = import_from_json(
        storage2.clone(),
        &export_path,
        ImportOptions::default(),
    )
    .await?;

    println!("   Re-imported vertices: {}", reimport_stats.vertices_imported);
    println!("   Re-imported edges: {}", reimport_stats.edges_imported);

    // Verify counts match
    let persons2 = storage2.scan_vertices("Person").await?;
    let languages2 = storage2.scan_vertices("Language").await?;
    assert_eq!(persons.len(), persons2.len());
    assert_eq!(languages.len(), languages2.len());

    println!("   ✓ Round-trip successful!\n");

    println!("=== Demo Complete ===");
    println!("\nKey takeaways:");
    println!("- JSON format supports both vertices and edges");
    println!("- Import options allow batch processing and error handling");
    println!("- Export preserves all graph data with labels and properties");
    println!("- Round-trip import/export maintains data integrity");

    Ok(())
}
