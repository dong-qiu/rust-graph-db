/// Storage engine demonstration
///
/// This example demonstrates:
/// 1. Creating a RocksDB storage instance
/// 2. Performing CRUD operations on vertices and edges
/// 3. Querying relationships
/// 4. Using transactions

use rust_graph_db::storage::rocksdb_store::RocksDbStorage;
use rust_graph_db::{GraphStorage, GraphTransaction};
use serde_json::json;
use tempfile::TempDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Storage Engine Demonstration ===\n");

    // Create a temporary directory for the database
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path();

    println!("1. Creating RocksDB Storage");
    println!("{}", "-".repeat(50));
    let storage = RocksDbStorage::new(db_path, "demo_graph")?;
    println!("✓ Storage created at: {:?}\n", db_path);

    // Example 1: Create vertices
    println!("2. Creating Vertices");
    println!("{}", "-".repeat(50));

    let alice = storage
        .create_vertex("Person", json!({
            "name": "Alice",
            "age": 30,
            "city": "Beijing"
        }))
        .await?;
    println!("✓ Created vertex: {} ({})", alice.id, alice.label);
    println!("  Properties: {:?}", alice.properties);

    let bob = storage
        .create_vertex("Person", json!({
            "name": "Bob",
            "age": 25,
            "city": "Shanghai"
        }))
        .await?;
    println!("✓ Created vertex: {} ({})", bob.id, bob.label);

    let acme = storage
        .create_vertex("Company", json!({
            "name": "ACME Corp",
            "industry": "Technology"
        }))
        .await?;
    println!("✓ Created vertex: {} ({})\n", acme.id, acme.label);

    // Example 2: Create edges
    println!("3. Creating Edges (Relationships)");
    println!("{}", "-".repeat(50));

    let knows_edge = storage
        .create_edge("KNOWS", alice.id, bob.id, json!({
            "since": 2020,
            "context": "university"
        }))
        .await?;
    println!("✓ Created edge: {} -> {}", alice.id, bob.id);
    println!("  Type: {}", knows_edge.label);

    let works_for_edge = storage
        .create_edge("WORKS_FOR", alice.id, acme.id, json!({
            "since": 2022,
            "position": "Engineer"
        }))
        .await?;
    println!("✓ Created edge: {} -> {}", alice.id, acme.id);
    println!("  Type: {}\n", works_for_edge.label);

    // Example 3: Query vertices
    println!("4. Querying Vertices");
    println!("{}", "-".repeat(50));

    let retrieved = storage.get_vertex(alice.id).await?;
    if let Some(v) = retrieved {
        println!("✓ Retrieved vertex: {}", v.id);
        println!("  Name: {}", v.get_property("name").unwrap());
        println!("  Age: {}", v.get_property("age").unwrap());
    }

    let people = storage.scan_vertices("Person").await?;
    println!("✓ Scanned {} Person vertices", people.len());

    let companies = storage.scan_vertices("Company").await?;
    println!("✓ Scanned {} Company vertices\n", companies.len());

    // Example 4: Query relationships
    println!("5. Querying Relationships");
    println!("{}", "-".repeat(50));

    let outgoing = storage.get_outgoing_edges(alice.id).await?;
    println!("✓ Alice has {} outgoing relationships:", outgoing.len());
    for edge in &outgoing {
        println!("  - {} -> {}: {}", edge.start, edge.end, edge.label);
    }

    let incoming = storage.get_incoming_edges(bob.id).await?;
    println!("✓ Bob has {} incoming relationships:", incoming.len());
    for edge in &incoming {
        println!("  - {} -> {}: {}", edge.start, edge.end, edge.label);
    }
    println!();

    // Example 5: Using transactions
    println!("6. Using Transactions");
    println!("{}", "-".repeat(50));

    let mut tx = storage.begin_transaction().await?;

    let carol = tx
        .create_vertex("Person", json!({
            "name": "Carol",
            "age": 28
        }))
        .await?;
    println!("✓ Created vertex in transaction: {}", carol.id);

    tx.create_edge("KNOWS", bob.id, carol.id, json!({
        "since": 2021
    }))
    .await?;
    println!("✓ Created edge in transaction: {} -> {}", bob.id, carol.id);

    tx.commit().await?;
    println!("✓ Transaction committed\n");

    // Verify transaction was persisted
    let carol_retrieved = storage.get_vertex(carol.id).await?;
    assert!(carol_retrieved.is_some());
    println!("✓ Verified transaction was persisted");

    // Example 6: Transaction rollback
    println!("\n7. Transaction Rollback");
    println!("{}", "-".repeat(50));

    let mut tx2 = storage.begin_transaction().await?;

    let temp_vertex = tx2
        .create_vertex("Person", json!({
            "name": "TempUser"
        }))
        .await?;
    println!("✓ Created temporary vertex: {}", temp_vertex.id);

    tx2.rollback().await?;
    println!("✓ Transaction rolled back");

    let temp_retrieved = storage.get_vertex(temp_vertex.id).await?;
    assert!(temp_retrieved.is_none());
    println!("✓ Verified vertex was not persisted\n");

    // Example 7: Delete operations
    println!("8. Delete Operations");
    println!("{}", "-".repeat(50));

    // Try to delete vertex with edges (should fail)
    let result = storage.delete_vertex(alice.id).await;
    if result.is_err() {
        println!("✓ Cannot delete vertex with edges (as expected)");
    }

    // Delete edge first
    storage.delete_edge(knows_edge.id).await?;
    println!("✓ Deleted edge: {}", knows_edge.id);

    let outgoing_after = storage.get_outgoing_edges(alice.id).await?;
    println!("✓ Alice now has {} outgoing relationships\n", outgoing_after.len());

    // Statistics
    println!("9. Final Statistics");
    println!("{}", "-".repeat(50));

    let all_people = storage.scan_vertices("Person").await?;
    let all_companies = storage.scan_vertices("Company").await?;
    let all_edges = storage.scan_edges("KNOWS").await?
        .len()
        + storage.scan_edges("WORKS_FOR").await?.len();

    println!("✓ Total People: {}", all_people.len());
    println!("✓ Total Companies: {}", all_companies.len());
    println!("✓ Total Relationships: {}", all_edges);

    println!("\n=== Demonstration Complete ===");
    println!("Database will be cleaned up on exit");

    Ok(())
}
