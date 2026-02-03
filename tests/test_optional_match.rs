/// Integration tests for OPTIONAL MATCH clause
///
/// OPTIONAL MATCH implements LEFT JOIN semantics - when a pattern doesn't match,
/// the row is preserved with NULL values for unmatched variables.

use rust_graph_db::executor::{QueryExecutor, Value};
use rust_graph_db::parser::parse_cypher;
use rust_graph_db::storage::rocksdb_store::RocksDbStorage;
use rust_graph_db::storage::GraphStorage;
use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;

async fn setup_test_db() -> (Arc<RocksDbStorage>, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let storage = Arc::new(RocksDbStorage::new(temp_dir.path(), "test_graph").unwrap());

    // Create test data directly using storage
    // Alice -> Bob -> Charlie
    // David (no connections)

    // Create vertices
    let alice = storage.create_vertex("Person", json!({"name": "Alice", "age": 30})).await.unwrap();
    let bob = storage.create_vertex("Person", json!({"name": "Bob", "age": 25})).await.unwrap();
    let charlie = storage.create_vertex("Person", json!({"name": "Charlie", "age": 35})).await.unwrap();
    let david = storage.create_vertex("Person", json!({"name": "David", "age": 28})).await.unwrap();

    // Create edges: Alice -> Bob -> Charlie
    storage.create_edge("KNOWS", alice.id, bob.id, json!({"since": 2020})).await.unwrap();
    storage.create_edge("KNOWS", bob.id, charlie.id, json!({"since": 2021})).await.unwrap();

    (storage, temp_dir)
}

#[tokio::test]
async fn test_optional_match_basic() {
    let (storage, _temp_dir) = setup_test_db().await;
    let executor = QueryExecutor::new(storage);

    // Test OPTIONAL MATCH with independent patterns first
    // This tests the basic structure without variable binding complexity
    let query = parse_cypher(
        r#"MATCH (p:Person {name: "Alice"})
           OPTIONAL MATCH (x:Person {name: "NonExistent"})
           RETURN p.name, x.name"#,
    )
    .unwrap();

    let results = executor.execute(query).await.unwrap();

    // Should return 1 row: Alice with NULL for x.name
    assert_eq!(results.len(), 1);

    // x.name should be NULL since NonExistent doesn't exist
    if let Some(row) = results.first() {
        // p.name should be Alice
        if let Some(Value::String(name)) = row.get("p.name") {
            assert_eq!(name, "Alice");
        } else {
            panic!("Expected p.name to be String(\"Alice\")");
        }

        // x.name should be NULL since the OPTIONAL MATCH found no match
        assert!(matches!(row.get("x.name"), Some(&Value::Null)));
    }
}

#[tokio::test]
async fn test_optional_match_no_results() {
    let (storage, _temp_dir) = setup_test_db().await;
    let executor = QueryExecutor::new(storage);

    // OPTIONAL MATCH with no matches should return rows with NULL values
    let query = parse_cypher(
        r#"MATCH (p:Person {name: "Alice"})
           OPTIONAL MATCH (x:Person {name: "Nobody"})
           OPTIONAL MATCH (y:Person {name: "NooneElse"})
           RETURN p.name, x.name, y.name"#,
    )
    .unwrap();

    let results = executor.execute(query).await.unwrap();

    // Should return 1 row with Alice, NULL, NULL
    assert_eq!(results.len(), 1);

    if let Some(row) = results.first() {
        assert_eq!(row.get("p.name"), Some(&Value::String("Alice".to_string())));
        assert_eq!(row.get("x.name"), Some(&Value::Null));
        assert_eq!(row.get("y.name"), Some(&Value::Null));
    }
}

// NOTE: The following tests require variable binding between MATCH clauses,
// which is not yet implemented. They are disabled for now.
// TODO: Implement variable binding support in OPTIONAL MATCH

#[tokio::test]
#[ignore = "Requires variable binding support"]
async fn test_multiple_optional_match() {
    let (storage, _temp_dir) = setup_test_db().await;
    let executor = QueryExecutor::new(storage);

    // Multiple OPTIONAL MATCH clauses
    let query = parse_cypher(
        r#"MATCH (p:Person {name: "Alice"})
           OPTIONAL MATCH (p)-[:KNOWS]->(friend1:Person)
           OPTIONAL MATCH (friend1)-[:KNOWS]->(friend2:Person)
           RETURN p.name, friend1.name, friend2.name"#,
    )
    .unwrap();

    let results = executor.execute(query).await.unwrap();

    // Alice knows Bob, Bob knows Charlie
    // So we should get: Alice -> Bob -> Charlie
    assert!(!results.is_empty());

    // friend1 should be Bob
    let friend1 = results[0].get("friend1").unwrap();
    assert!(!matches!(friend1, Value::Null));

    // friend2 should be Charlie
    let friend2 = results[0].get("friend2").unwrap();
    assert!(!matches!(friend2, Value::Null));
}

#[tokio::test]
#[ignore = "Requires variable binding support"]
async fn test_optional_match_with_where() {
    let (storage, _temp_dir) = setup_test_db().await;
    let executor = QueryExecutor::new(storage);

    // OPTIONAL MATCH with WHERE on the optional part
    let query = parse_cypher(
        r#"MATCH (p:Person)
           OPTIONAL MATCH (p)-[r:KNOWS]->(friend:Person)
           WHERE friend.age > 30
           RETURN p.name, friend.name"#,
    )
    .unwrap();

    let results = executor.execute(query).await.unwrap();

    // Should return all persons
    assert_eq!(results.len(), 4);

    // Only Bob->Charlie should have a friend (Charlie's age is 35 > 30)
    let with_qualifying_friends = results
        .iter()
        .filter(|row| !matches!(row.get("friend"), Some(Value::Null)))
        .count();
    assert_eq!(with_qualifying_friends, 1);
}

#[tokio::test]
#[ignore = "Requires variable binding support"]
async fn test_optional_match_null_handling() {
    let (storage, _temp_dir) = setup_test_db().await;
    let executor = QueryExecutor::new(storage);

    // Query person without connections
    let query = parse_cypher(
        r#"MATCH (p:Person {name: "David"})
           OPTIONAL MATCH (p)-[r:KNOWS]->(friend:Person)
           RETURN p.name, r, friend"#,
    )
    .unwrap();

    let results = executor.execute(query).await.unwrap();

    assert_eq!(results.len(), 1);

    // David has no relationships, so r and friend should be NULL
    assert_eq!(results[0].get("p").is_some(), true);
    assert!(matches!(results[0].get("r"), Some(Value::Null)));
    assert!(matches!(results[0].get("friend"), Some(Value::Null)));
}

#[tokio::test]
#[ignore = "Requires variable binding support"]
async fn test_optional_match_vs_required_match() {
    let (storage, _temp_dir) = setup_test_db().await;
    let executor = QueryExecutor::new(storage);

    // Required MATCH - should only return rows with matches
    let required = parse_cypher(
        r#"MATCH (p:Person)
           MATCH (p)-[:KNOWS]->(friend:Person)
           RETURN p.name"#,
    )
    .unwrap();

    let required_results = executor.execute(required).await.unwrap();
    assert_eq!(required_results.len(), 2); // Only Alice and Bob have KNOWS edges

    // OPTIONAL MATCH - should return all persons
    let optional = parse_cypher(
        r#"MATCH (p:Person)
           OPTIONAL MATCH (p)-[:KNOWS]->(friend:Person)
           RETURN p.name"#,
    )
    .unwrap();

    let optional_results = executor.execute(optional).await.unwrap();
    assert_eq!(optional_results.len(), 4); // All 4 persons

    // OPTIONAL MATCH should return more results than required MATCH
    assert!(optional_results.len() > required_results.len());
}

#[tokio::test]
#[ignore = "Requires variable binding support"]
async fn test_optional_match_complex_pattern() {
    let (storage, _temp_dir) = setup_test_db().await;
    let executor = QueryExecutor::new(storage);

    // Complex pattern with multiple nodes and edges
    let query = parse_cypher(
        r#"MATCH (p:Person {name: "Charlie"})
           OPTIONAL MATCH (p)-[:KNOWS]->(x:Person)-[:KNOWS]->(y:Person)
           RETURN p.name, x.name, y.name"#,
    )
    .unwrap();

    let results = executor.execute(query).await.unwrap();

    // Charlie has no outgoing KNOWS edges, so x and y should be NULL
    assert_eq!(results.len(), 1);
    assert!(matches!(results[0].get("x"), Some(Value::Null)));
    assert!(matches!(results[0].get("y"), Some(Value::Null)));
}
