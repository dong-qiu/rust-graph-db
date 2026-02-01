/// DELETE clause executor
///
/// Implements vertex and edge deletion for Cypher DELETE queries.

use super::{ExecutionError, ExecutionResult, Row, Value};
use crate::parser::ast::*;
use crate::storage::GraphStorage;
use crate::types::Graphid;
use std::sync::Arc;

pub struct DeleteExecutor {
    storage: Arc<dyn GraphStorage>,
}

impl DeleteExecutor {
    pub fn new(storage: Arc<dyn GraphStorage>) -> Self {
        Self { storage }
    }

    /// Execute DELETE clause (standalone)
    pub async fn execute(
        &mut self,
        _expressions: &[Expression],
        _detach: bool,
    ) -> ExecutionResult<()> {
        let mut tx = self.storage.begin_transaction().await?;

        for _expr in _expressions {
            // For standalone DELETE, expressions should be simple identifiers
            // In practice, this requires a prior MATCH to bind variables
            // For now, we'll return an error
            return Err(ExecutionError::InvalidExpression(
                "DELETE requires prior MATCH to bind variables".to_string(),
            ));
        }

        tx.commit().await?;
        Ok(())
    }

    /// Execute DELETE with context from MATCH
    pub async fn execute_with_context(
        &mut self,
        expressions: &[Expression],
        detach: bool,
        rows: &[Row],
    ) -> ExecutionResult<()> {
        let mut tx = self.storage.begin_transaction().await?;

        for row in rows {
            for expr in expressions {
                match expr {
                    Expression::Variable(var) => {
                        let value = row
                            .get(var)
                            .ok_or_else(|| ExecutionError::VariableNotFound(var.clone()))?;

                        match value {
                            Value::Vertex(v) => {
                                if detach {
                                    self.detach_delete_vertex(&mut tx, v.id).await?;
                                } else {
                                    self.delete_vertex(&mut tx, v.id).await?;
                                }
                            }
                            Value::Edge(e) => {
                                self.delete_edge(&mut tx, e.id).await?;
                            }
                            _ => {
                                return Err(ExecutionError::TypeMismatch {
                                    expected: "Vertex or Edge".to_string(),
                                    actual: format!("{:?}", value),
                                });
                            }
                        }
                    }
                    _ => {
                        return Err(ExecutionError::UnsupportedOperation(
                            "Only variables supported in DELETE".to_string(),
                        ));
                    }
                }
            }
        }

        tx.commit().await?;
        Ok(())
    }

    async fn delete_vertex(
        &self,
        tx: &mut Box<dyn crate::storage::GraphTransaction>,
        id: Graphid,
    ) -> ExecutionResult<()> {
        // Check if vertex has edges
        let outgoing = tx.get_outgoing_edges(id).await?;
        let incoming = tx.get_incoming_edges(id).await?;

        if !outgoing.is_empty() || !incoming.is_empty() {
            return Err(ExecutionError::InvalidExpression(
                "Cannot delete vertex with edges (use DETACH DELETE)".to_string(),
            ));
        }

        tx.delete_vertex(id).await?;
        Ok(())
    }

    async fn detach_delete_vertex(
        &self,
        tx: &mut Box<dyn crate::storage::GraphTransaction>,
        id: Graphid,
    ) -> ExecutionResult<()> {
        // Delete all connected edges first
        let outgoing = tx.get_outgoing_edges(id).await?;
        let incoming = tx.get_incoming_edges(id).await?;

        for edge in outgoing {
            tx.delete_edge(edge.id).await?;
        }

        for edge in incoming {
            tx.delete_edge(edge.id).await?;
        }

        // Then delete the vertex
        tx.delete_vertex(id).await?;
        Ok(())
    }

    async fn delete_edge(
        &self,
        tx: &mut Box<dyn crate::storage::GraphTransaction>,
        id: Graphid,
    ) -> ExecutionResult<()> {
        tx.delete_edge(id).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{rocksdb_store::RocksDbStorage, GraphStorage};
    use tempfile::TempDir;

    async fn setup_test_storage() -> (Arc<dyn GraphStorage>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage: Arc<dyn GraphStorage> = Arc::new(RocksDbStorage::new(temp_dir.path(), "test_graph").unwrap());
        (storage, temp_dir)
    }

    #[tokio::test]
    async fn test_delete_vertex_no_edges() {
        let (storage, _temp) = setup_test_storage().await;

        // Create vertex
        let mut tx = storage.begin_transaction().await.unwrap();
        let vertex = tx
            .create_vertex("Person", serde_json::json!({"name": "Alice"}))
            .await
            .unwrap();
        tx.commit().await.unwrap();

        // Delete it
        let mut executor = DeleteExecutor::new(storage.clone());
        let row = Row::new().with_binding("n".to_string(), Value::Vertex(vertex.clone()));

        executor
            .execute_with_context(&[Expression::Variable("n".to_string())], false, &[row])
            .await
            .unwrap();

        // Verify deleted
        let result = storage.get_vertex(vertex.id).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_delete_vertex_with_edges_fails() {
        let (storage, _temp) = setup_test_storage().await;

        // Create vertices and edge
        let mut tx = storage.begin_transaction().await.unwrap();
        let alice = tx
            .create_vertex("Person", serde_json::json!({"name": "Alice"}))
            .await
            .unwrap();
        let bob = tx
            .create_vertex("Person", serde_json::json!({"name": "Bob"}))
            .await
            .unwrap();
        tx.create_edge("KNOWS", alice.id, bob.id, serde_json::json!({}))
            .await
            .unwrap();
        tx.commit().await.unwrap();

        // Try to delete Alice (should fail)
        let mut executor = DeleteExecutor::new(storage.clone());
        let row = Row::new().with_binding("n".to_string(), Value::Vertex(alice.clone()));

        let result = executor
            .execute_with_context(&[Expression::Variable("n".to_string())], false, &[row])
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_detach_delete_vertex() {
        let (storage, _temp) = setup_test_storage().await;

        // Create vertices and edge
        let mut tx = storage.begin_transaction().await.unwrap();
        let alice = tx
            .create_vertex("Person", serde_json::json!({"name": "Alice"}))
            .await
            .unwrap();
        let bob = tx
            .create_vertex("Person", serde_json::json!({"name": "Bob"}))
            .await
            .unwrap();
        let edge = tx
            .create_edge("KNOWS", alice.id, bob.id, serde_json::json!({}))
            .await
            .unwrap();
        tx.commit().await.unwrap();

        // DETACH DELETE Alice
        let mut executor = DeleteExecutor::new(storage.clone());
        let row = Row::new().with_binding("n".to_string(), Value::Vertex(alice.clone()));

        executor
            .execute_with_context(&[Expression::Variable("n".to_string())], true, &[row])
            .await
            .unwrap();

        // Verify Alice is deleted
        let result = storage.get_vertex(alice.id).await.unwrap();
        assert!(result.is_none());

        // Verify edge is deleted
        let result = storage.get_edge(edge.id).await.unwrap();
        assert!(result.is_none());

        // Verify Bob still exists
        let result = storage.get_vertex(bob.id).await.unwrap();
        assert!(result.is_some());
    }
}
