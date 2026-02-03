/// Integration test for SET with nested properties
/// Tests the complete flow from parsing to execution

use rust_graph_db::executor::*;
use rust_graph_db::parser::parse_cypher;
use rust_graph_db::parser::ast::*;
use rust_graph_db::storage::{rocksdb_store::RocksDbStorage, GraphStorage};
use std::sync::Arc;
use tempfile::TempDir;

async fn setup_test_storage() -> (Arc<dyn GraphStorage>, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let storage: Arc<dyn GraphStorage> =
        Arc::new(RocksDbStorage::new(temp_dir.path(), "test_graph").unwrap());
    (storage, temp_dir)
}

#[tokio::test]
async fn test_set_nested_property_end_to_end() {
    let (storage, _temp) = setup_test_storage().await;

    // Create a vertex with nested properties
    let mut tx = storage.begin_transaction().await.unwrap();
    let vertex = tx
        .create_vertex(
            "Person",
            serde_json::json!({
                "name": "Alice",
                "address": {
                    "city": "Beijing",
                    "country": "China"
                }
            }),
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();

    // Parse SET query with nested property
    let query = "MATCH (n:Person) SET n.address.city = 'Shanghai';";
    let parsed = parse_cypher(query).unwrap();

    // Verify parsing
    if let CypherQuery::Mixed {
        write_clause: WriteClause::Set { items },
        ..
    } = &parsed
    {
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].property.base, "n");
        assert_eq!(items[0].property.properties, vec!["address", "city"]);
    } else {
        panic!("Expected Mixed query with SET clause");
    }

    // Execute SET
    let mut set_executor = SetExecutor::new(storage.clone());

    // Create context row
    let row = Row::new().with_binding("n".to_string(), Value::Vertex(vertex.clone()));

    if let CypherQuery::Mixed {
        write_clause: WriteClause::Set { items },
        ..
    } = parsed
    {
        set_executor
            .execute_with_context(&items, &[row])
            .await
            .unwrap();
    }

    // Verify the update
    let updated = storage.get_vertex(vertex.id).await.unwrap().unwrap();
    assert_eq!(updated.properties["address"]["city"], "Shanghai");
    assert_eq!(updated.properties["address"]["country"], "China");
    assert_eq!(updated.properties["name"], "Alice");
}

#[tokio::test]
async fn test_set_deep_nested_property() {
    let (storage, _temp) = setup_test_storage().await;

    // Create a vertex with deeply nested properties
    let mut tx = storage.begin_transaction().await.unwrap();
    let vertex = tx
        .create_vertex(
            "Person",
            serde_json::json!({
                "name": "Bob",
                "contact": {
                    "address": {
                        "city": "Beijing",
                        "street": "Main Street"
                    },
                    "phone": "123-456-7890"
                }
            }),
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();

    // Parse SET query with deep nested property
    let query = "MATCH (n:Person) SET n.contact.address.city = 'Shenzhen';";
    let parsed = parse_cypher(query).unwrap();

    // Verify parsing
    if let CypherQuery::Mixed {
        write_clause: WriteClause::Set { items },
        ..
    } = &parsed
    {
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].property.base, "n");
        assert_eq!(
            items[0].property.properties,
            vec!["contact", "address", "city"]
        );
    } else {
        panic!("Expected Mixed query with SET clause");
    }

    // Execute SET
    let mut set_executor = SetExecutor::new(storage.clone());
    let row = Row::new().with_binding("n".to_string(), Value::Vertex(vertex.clone()));

    if let CypherQuery::Mixed {
        write_clause: WriteClause::Set { items },
        ..
    } = parsed
    {
        set_executor
            .execute_with_context(&items, &[row])
            .await
            .unwrap();
    }

    // Verify the update
    let updated = storage.get_vertex(vertex.id).await.unwrap().unwrap();
    assert_eq!(updated.properties["contact"]["address"]["city"], "Shenzhen");
    assert_eq!(
        updated.properties["contact"]["address"]["street"],
        "Main Street"
    );
    assert_eq!(updated.properties["contact"]["phone"], "123-456-7890");
    assert_eq!(updated.properties["name"], "Bob");
}

#[tokio::test]
async fn test_set_multiple_nested_properties() {
    let (storage, _temp) = setup_test_storage().await;

    // Create a vertex
    let mut tx = storage.begin_transaction().await.unwrap();
    let vertex = tx
        .create_vertex(
            "Person",
            serde_json::json!({
                "name": "Charlie",
                "profile": {
                    "age": 30,
                    "city": "Beijing"
                }
            }),
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();

    // Parse SET query with multiple nested properties
    let query = "MATCH (n:Person) SET n.profile.age = 31, n.profile.city = 'Shanghai';";
    let parsed = parse_cypher(query).unwrap();

    // Verify parsing
    if let CypherQuery::Mixed {
        write_clause: WriteClause::Set { items },
        ..
    } = &parsed
    {
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].property.properties, vec!["profile", "age"]);
        assert_eq!(items[1].property.properties, vec!["profile", "city"]);
    } else {
        panic!("Expected Mixed query with SET clause");
    }

    // Execute SET
    let mut set_executor = SetExecutor::new(storage.clone());
    let row = Row::new().with_binding("n".to_string(), Value::Vertex(vertex.clone()));

    if let CypherQuery::Mixed {
        write_clause: WriteClause::Set { items },
        ..
    } = parsed
    {
        set_executor
            .execute_with_context(&items, &[row])
            .await
            .unwrap();
    }

    // Verify the updates
    let updated = storage.get_vertex(vertex.id).await.unwrap().unwrap();
    assert_eq!(updated.properties["profile"]["age"], 31);
    assert_eq!(updated.properties["profile"]["city"], "Shanghai");
    assert_eq!(updated.properties["name"], "Charlie");
}
