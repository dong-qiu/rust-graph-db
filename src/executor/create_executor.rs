/// CREATE clause executor
///
/// Implements vertex and edge creation for Cypher CREATE queries.

use super::{ExecutionError, ExecutionResult, Row, Value};
use crate::parser::ast::*;
use crate::storage::GraphStorage;
use crate::types::Graphid;
use std::collections::HashMap;
use std::sync::Arc;

pub struct CreateExecutor {
    storage: Arc<dyn GraphStorage>,
}

impl CreateExecutor {
    pub fn new(storage: Arc<dyn GraphStorage>) -> Self {
        Self { storage }
    }

    /// Execute CREATE clause
    pub async fn execute(&mut self, patterns: &[Pattern]) -> ExecutionResult<Vec<Row>> {
        let mut tx = self.storage.begin_transaction().await?;
        let mut created_bindings: HashMap<String, Value> = HashMap::new();
        let mut results = Vec::new();

        for pattern in patterns {
            self.create_pattern(&mut tx, pattern, &mut created_bindings)
                .await?;
        }

        tx.commit().await?;

        // Return created entities
        if !created_bindings.is_empty() {
            let mut row = Row::new();
            row.bindings = created_bindings;
            results.push(row);
        }

        Ok(results)
    }

    async fn create_pattern(
        &self,
        tx: &mut Box<dyn crate::storage::GraphTransaction>,
        pattern: &Pattern,
        bindings: &mut HashMap<String, Value>,
    ) -> ExecutionResult<()> {
        let mut last_vertex_id: Option<Graphid> = None;
        let mut skip_next = false;

        for element in &pattern.elements {
            if skip_next {
                skip_next = false;
                continue;
            }

            match element {
                PatternElement::Node(node) => {
                    let vertex = self.create_node(tx, node).await?;
                    last_vertex_id = Some(vertex.id);

                    if let Some(var) = &node.variable {
                        bindings.insert(var.clone(), Value::Vertex(vertex));
                    }
                }

                PatternElement::Edge(edge) => {
                    // Edge must follow a node
                    if let Some(start_id) = last_vertex_id {
                        // Find the next node
                        let next_node = self.find_next_node(pattern, element)?;
                        let end_vertex = self.create_node(tx, next_node).await?;

                        // Create edge based on direction
                        let (actual_start, actual_end) = match edge.direction {
                            Direction::Right => (start_id, end_vertex.id),
                            Direction::Left => (end_vertex.id, start_id),
                            Direction::Both => {
                                return Err(ExecutionError::InvalidExpression(
                                    "Cannot CREATE undirected edges".to_string(),
                                ));
                            }
                        };

                        let edge_entity = self.create_edge(tx, edge, actual_start, actual_end).await?;

                        if let Some(var) = &edge.variable {
                            bindings.insert(var.clone(), Value::Edge(edge_entity));
                        }

                        if let Some(var) = &next_node.variable {
                            bindings.insert(var.clone(), Value::Vertex(end_vertex.clone()));
                        }

                        last_vertex_id = Some(end_vertex.id);

                        // Skip the next node since we already created it
                        skip_next = true;
                    } else {
                        return Err(ExecutionError::InvalidExpression(
                            "Edge must follow a node".to_string(),
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    async fn create_node(
        &self,
        tx: &mut Box<dyn crate::storage::GraphTransaction>,
        node: &NodePattern,
    ) -> ExecutionResult<crate::types::Vertex> {
        let label = node
            .label
            .as_ref()
            .ok_or_else(|| ExecutionError::InvalidExpression("Node must have a label".to_string()))?;

        let properties = if let Some(props) = &node.properties {
            self.evaluate_properties(props)?
        } else {
            serde_json::json!({})
        };

        let vertex = tx.create_vertex(label, properties).await?;
        Ok(vertex)
    }

    async fn create_edge(
        &self,
        tx: &mut Box<dyn crate::storage::GraphTransaction>,
        edge: &EdgePattern,
        start: Graphid,
        end: Graphid,
    ) -> ExecutionResult<crate::types::Edge> {
        let label = edge
            .label
            .as_ref()
            .ok_or_else(|| ExecutionError::InvalidExpression("Edge must have a label".to_string()))?;

        let properties = if let Some(props) = &edge.properties {
            self.evaluate_properties(props)?
        } else {
            serde_json::json!({})
        };

        let edge_entity = tx.create_edge(label, start, end, properties).await?;
        Ok(edge_entity)
    }

    fn find_next_node<'a>(
        &self,
        pattern: &'a Pattern,
        current_edge: &PatternElement,
    ) -> ExecutionResult<&'a NodePattern> {
        let mut found_edge = false;

        for element in &pattern.elements {
            if found_edge {
                if let PatternElement::Node(node) = element {
                    return Ok(node);
                }
            }

            if std::ptr::eq(element, current_edge) {
                found_edge = true;
            }
        }

        Err(ExecutionError::InvalidExpression(
            "Edge must be followed by a node".to_string(),
        ))
    }

    fn evaluate_properties(
        &self,
        props: &HashMap<String, Expression>,
    ) -> ExecutionResult<serde_json::Value> {
        let mut map = serde_json::Map::new();

        for (key, expr) in props {
            let value = self.evaluate_expression(expr)?;
            map.insert(key.clone(), value);
        }

        Ok(serde_json::Value::Object(map))
    }

    fn evaluate_expression(&self, expr: &Expression) -> ExecutionResult<serde_json::Value> {
        match expr {
            Expression::Literal(lit) => self.literal_to_json(lit),
            Expression::Parameter(name) => {
                // TODO: Support parameters
                Err(ExecutionError::UnsupportedOperation(format!(
                    "Parameter ${} not yet supported",
                    name
                )))
            }
            _ => Err(ExecutionError::InvalidExpression(
                "Complex expressions not supported in CREATE".to_string(),
            )),
        }
    }

    fn literal_to_json(&self, lit: &Literal) -> ExecutionResult<serde_json::Value> {
        match lit {
            Literal::Null => Ok(serde_json::Value::Null),
            Literal::Boolean(b) => Ok(serde_json::Value::Bool(*b)),
            Literal::Integer(i) => Ok(serde_json::Value::Number((*i).into())),
            Literal::Float(f) => {
                serde_json::Number::from_f64(*f)
                    .map(serde_json::Value::Number)
                    .ok_or_else(|| ExecutionError::InvalidExpression(
                        format!("Invalid float value: {}", f)
                    ))
            }
            Literal::String(s) => Ok(serde_json::Value::String(s.clone())),
            Literal::List(items) => {
                let values: ExecutionResult<Vec<_>> = items
                    .iter()
                    .map(|e| self.evaluate_expression(e))
                    .collect();
                Ok(serde_json::Value::Array(values?))
            }
            Literal::Map(map) => {
                let obj: ExecutionResult<serde_json::Map<String, serde_json::Value>> = map
                    .iter()
                    .map(|(k, v)| Ok((k.clone(), self.evaluate_expression(v)?)))
                    .collect();
                Ok(serde_json::Value::Object(obj?))
            }
        }
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
    async fn test_create_node() {
        let (storage, _temp) = setup_test_storage().await;

        let mut executor = CreateExecutor::new(storage.clone());

        let mut props = HashMap::new();
        props.insert(
            "name".to_string(),
            Expression::Literal(Literal::String("Alice".to_string())),
        );
        props.insert(
            "age".to_string(),
            Expression::Literal(Literal::Integer(30)),
        );

        let pattern = Pattern {
            elements: vec![PatternElement::Node(NodePattern {
                variable: Some("n".to_string()),
                label: Some("Person".to_string()),
                properties: Some(props),
            })],
        };

        let results = executor.execute(&[pattern]).await.unwrap();
        assert_eq!(results.len(), 1);

        // Verify created
        let vertices = storage.scan_vertices("Person").await.unwrap();
        assert_eq!(vertices.len(), 1);
        assert_eq!(vertices[0].properties["name"], "Alice");
        assert_eq!(vertices[0].properties["age"], 30);
    }

    #[tokio::test]
    async fn test_create_relationship() {
        let (storage, _temp) = setup_test_storage().await;

        let mut executor = CreateExecutor::new(storage.clone());

        // CREATE (a:Person {name: 'Alice'})-[:KNOWS]->(b:Person {name: 'Bob'})
        let mut alice_props = HashMap::new();
        alice_props.insert(
            "name".to_string(),
            Expression::Literal(Literal::String("Alice".to_string())),
        );

        let mut bob_props = HashMap::new();
        bob_props.insert(
            "name".to_string(),
            Expression::Literal(Literal::String("Bob".to_string())),
        );

        let pattern = Pattern {
            elements: vec![
                PatternElement::Node(NodePattern {
                    variable: Some("a".to_string()),
                    label: Some("Person".to_string()),
                    properties: Some(alice_props),
                }),
                PatternElement::Edge(EdgePattern {
                    variable: Some("r".to_string()),
                    label: Some("KNOWS".to_string()),
                    properties: None,
                    direction: Direction::Right,
                }),
                PatternElement::Node(NodePattern {
                    variable: Some("b".to_string()),
                    label: Some("Person".to_string()),
                    properties: Some(bob_props),
                }),
            ],
        };

        let results = executor.execute(&[pattern]).await.unwrap();
        assert_eq!(results.len(), 1);

        // Verify created
        let vertices = storage.scan_vertices("Person").await.unwrap();
        assert_eq!(vertices.len(), 2);

        let edges = storage.scan_edges("KNOWS").await.unwrap();
        assert_eq!(edges.len(), 1);
    }
}
