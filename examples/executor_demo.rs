/// Query executor demonstration
///
/// This example shows how to use the Cypher query executor to:
/// - Parse Cypher queries
/// - Execute MATCH, CREATE, DELETE, SET operations
/// - Work with query results

use rust_graph_db::{parse_cypher, QueryExecutor};
use rust_graph_db::storage::rocksdb_store::RocksDbStorage;
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Cypher Query Executor Demo ===\n");

    // Setup storage
    let temp_dir = TempDir::new()?;
    let storage = Arc::new(RocksDbStorage::new(temp_dir.path(), "demo_graph")?);
    let executor = QueryExecutor::new(storage.clone());

    // Example 1: CREATE vertices
    println!("1. Creating vertices...");
    let create_alice = "CREATE (:Person {name: 'Alice', age: 30})";
    let ast = parse_cypher(create_alice)?;
    executor.execute(ast).await?;
    println!("   Created: Alice");

    let create_bob = "CREATE (:Person {name: 'Bob', age: 25})";
    let ast = parse_cypher(create_bob)?;
    executor.execute(ast).await?;
    println!("   Created: Bob\n");

    // Example 2: CREATE relationship
    println!("2. Creating relationship...");
    let create_edge = "CREATE (:Person {name: 'Charlie', age: 35})-[:KNOWS {since: 2020}]->(:Person {name: 'Diana', age: 28})";
    let ast = parse_cypher(create_edge)?;
    let results = executor.execute(ast).await?;
    println!("   Created: Charlie -[KNOWS]-> Diana");
    println!("   Results: {} rows\n", results.len());

    // Example 3: MATCH all persons
    println!("3. Matching all persons...");
    let match_all = "MATCH (p:Person) RETURN p";
    let ast = parse_cypher(match_all)?;
    let results = executor.execute(ast).await?;
    println!("   Found {} persons:", results.len());
    for row in &results {
        if let Some(person) = row.get("p") {
            println!("   - {:?}", person);
        }
    }
    println!();

    // Example 4: MATCH with properties
    println!("4. Matching persons with specific properties...");
    let match_filtered = "MATCH (p:Person {name: 'Alice'}) RETURN p";
    let ast = parse_cypher(match_filtered)?;
    let results = executor.execute(ast).await?;
    println!("   Found {} person(s) named Alice\n", results.len());

    // Example 5: MATCH and SET
    println!("5. Updating properties...");
    let update = "MATCH (p:Person {name: 'Bob'}) SET p.age = 26";
    let ast = parse_cypher(update)?;
    executor.execute(ast).await?;
    println!("   Updated Bob's age to 26\n");

    // Example 6: MATCH and DELETE
    println!("6. Deleting a person...");
    let delete = "MATCH (p:Person {name: 'Alice'}) DELETE p";
    let ast = parse_cypher(delete)?;
    executor.execute(ast).await?;
    println!("   Deleted Alice\n");

    // Example 7: Final count
    println!("7. Final count...");
    let count = "MATCH (p:Person) RETURN p";
    let ast = parse_cypher(count)?;
    let results = executor.execute(ast).await?;
    println!("   Remaining persons: {}\n", results.len());

    println!("=== Demo Complete ===");

    Ok(())
}
