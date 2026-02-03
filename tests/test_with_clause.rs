/// Integration tests for WITH clause
///
/// Tests the complete flow from parsing to execution

use rust_graph_db::executor::*;
use rust_graph_db::parser::parse_cypher;
use rust_graph_db::storage::{rocksdb_store::RocksDbStorage, GraphStorage};
use std::sync::Arc;
use tempfile::TempDir;

async fn setup_test_storage() -> (Arc<dyn GraphStorage>, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let storage: Arc<dyn GraphStorage> =
        Arc::new(RocksDbStorage::new(temp_dir.path(), "test_graph").unwrap());
    (storage, temp_dir)
}

async fn setup_test_data(storage: Arc<dyn GraphStorage>) {
    let mut tx = storage.begin_transaction().await.unwrap();

    // Create some test people
    tx.create_vertex("Person", serde_json::json!({"name": "Alice", "age": 30, "city": "Beijing"}))
        .await
        .unwrap();
    tx.create_vertex("Person", serde_json::json!({"name": "Bob", "age": 25, "city": "Shanghai"}))
        .await
        .unwrap();
    tx.create_vertex("Person", serde_json::json!({"name": "Charlie", "age": 35, "city": "Beijing"}))
        .await
        .unwrap();
    tx.create_vertex("Person", serde_json::json!({"name": "David", "age": 20, "city": "Shenzhen"}))
        .await
        .unwrap();

    tx.commit().await.unwrap();
}

#[tokio::test]
async fn test_with_basic() {
    let (storage, _temp) = setup_test_storage().await;
    setup_test_data(storage.clone()).await;

    let query = "MATCH (p:Person) WITH p, p.age AS age RETURN p.name, age;";
    let parsed = parse_cypher(query).unwrap();

    let executor = QueryExecutor::new(storage);
    let results = executor.execute(parsed).await.unwrap();

    assert_eq!(results.len(), 4);
    // Check that all results have 'p.name' and 'age' bindings
    for row in &results {
        assert!(row.get("p.name").is_some() || row.get("name").is_some());
        assert!(row.get("age").is_some());
    }
}

#[tokio::test]
async fn test_with_where_filter() {
    let (storage, _temp) = setup_test_storage().await;
    setup_test_data(storage.clone()).await;

    let query = "MATCH (p:Person) WITH p, p.age AS age WHERE age > 25 RETURN p.name, age;";
    let parsed = parse_cypher(query).unwrap();

    let executor = QueryExecutor::new(storage);
    let results = executor.execute(parsed).await.unwrap();

    // Should only return Alice (30) and Charlie (35)
    assert_eq!(results.len(), 2);

    for row in &results {
        let age = row.get("age").unwrap();
        match age {
            Value::Integer(a) => assert!(*a > 25),
            _ => panic!("Expected integer age"),
        }
    }
}

#[tokio::test]
async fn test_with_projection() {
    let (storage, _temp) = setup_test_storage().await;
    setup_test_data(storage.clone()).await;

    // WITH should project only selected variables
    let query = "MATCH (p:Person) WITH p.name AS name RETURN name;";
    let parsed = parse_cypher(query).unwrap();

    let executor = QueryExecutor::new(storage);
    let results = executor.execute(parsed).await.unwrap();

    assert_eq!(results.len(), 4);

    for row in &results {
        // Should only have 'name' binding, not 'p' or other fields
        assert!(row.get("name").is_some());
        // The original 'p' should not be accessible after WITH
        assert_eq!(row.bindings.len(), 1);
    }
}

#[tokio::test]
async fn test_with_order_limit() {
    let (storage, _temp) = setup_test_storage().await;
    setup_test_data(storage.clone()).await;

    let query = "MATCH (p:Person) WITH p, p.age AS age ORDER BY age DESC LIMIT 2 RETURN p.name, age;";
    let parsed = parse_cypher(query).unwrap();

    let executor = QueryExecutor::new(storage);
    let results = executor.execute(parsed).await.unwrap();

    // Should return top 2 by age: Charlie (35) and Alice (30)
    assert_eq!(results.len(), 2);

    // Verify ordering (descending by age)
    let ages: Vec<i64> = results
        .iter()
        .map(|row| match row.get("age").or_else(|| row.get("p.age")) {
            Some(Value::Integer(a)) => *a,
            _ => panic!("Expected integer age"),
        })
        .collect();

    // With DESC order, ages should be descending
    assert!(ages[0] >= ages[1], "Ages should be descending: {:?}", ages);
    assert!(ages[0] >= 30); // Charlie (35) or Alice (30)
}

#[tokio::test]
async fn test_with_multiple_filters() {
    let (storage, _temp) = setup_test_storage().await;
    setup_test_data(storage.clone()).await;

    // First filter in MATCH WHERE, then filter in WITH WHERE
    let query = "MATCH (p:Person) WHERE p.age >= 25 WITH p, p.age AS age WHERE age <= 30 RETURN p.name;";
    let parsed = parse_cypher(query).unwrap();

    let executor = QueryExecutor::new(storage);
    let results = executor.execute(parsed).await.unwrap();

    // Should return only those with 25 <= age <= 30: Alice (30) and Bob (25)
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_with_alias_in_return() {
    let (storage, _temp) = setup_test_storage().await;
    setup_test_data(storage.clone()).await;

    let query = "MATCH (p:Person) WITH p.name AS person_name, p.age AS person_age RETURN person_name, person_age;";
    let parsed = parse_cypher(query).unwrap();

    let executor = QueryExecutor::new(storage);
    let results = executor.execute(parsed).await.unwrap();

    assert_eq!(results.len(), 4);

    for row in &results {
        assert!(row.get("person_name").is_some());
        assert!(row.get("person_age").is_some());
    }
}
