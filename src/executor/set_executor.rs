/// SET clause executor
///
/// Implements property updates for Cypher SET queries.

use super::{ExecutionError, ExecutionResult, Row, Value};
use crate::parser::ast::*;
use crate::storage::GraphStorage;
use crate::types::Graphid;
use std::sync::Arc;

pub struct SetExecutor {
    storage: Arc<dyn GraphStorage>,
}

impl SetExecutor {
    pub fn new(storage: Arc<dyn GraphStorage>) -> Self {
        Self { storage }
    }

    /// Execute SET clause (standalone)
    pub async fn execute(&mut self, _items: &[SetItem]) -> ExecutionResult<()> {
        // Standalone SET requires prior MATCH context
        Err(ExecutionError::InvalidExpression(
            "SET requires prior MATCH to bind variables".to_string(),
        ))
    }

    /// Execute SET with context from MATCH
    pub async fn execute_with_context(
        &mut self,
        items: &[SetItem],
        rows: &[Row],
    ) -> ExecutionResult<()> {
        let mut tx = self.storage.begin_transaction().await?;

        for row in rows {
            for item in items {
                self.apply_set_item(&mut tx, item, row).await?;
            }
        }

        tx.commit().await?;
        Ok(())
    }

    async fn apply_set_item(
        &self,
        tx: &mut Box<dyn crate::storage::GraphTransaction>,
        item: &SetItem,
        row: &Row,
    ) -> ExecutionResult<()> {
        let prop_expr = &item.property;
        let value_expr = &item.value;

        // Get the entity (vertex or edge)
        let entity_var = &prop_expr.base;
        let entity_value = row
            .get(entity_var)
            .ok_or_else(|| ExecutionError::VariableNotFound(entity_var.clone()))?;

        // Evaluate the new value
        let new_value = self.evaluate_expression(value_expr, row)?;

        // Update the entity
        match entity_value {
            Value::Vertex(v) => {
                self.update_vertex_property(tx, v.id, &prop_expr.properties, new_value)
                    .await?;
            }
            Value::Edge(e) => {
                self.update_edge_property(tx, e.id, &prop_expr.properties, new_value)
                    .await?;
            }
            _ => {
                return Err(ExecutionError::TypeMismatch {
                    expected: "Vertex or Edge".to_string(),
                    actual: format!("{:?}", entity_value),
                });
            }
        }

        Ok(())
    }

    async fn update_vertex_property(
        &self,
        tx: &mut Box<dyn crate::storage::GraphTransaction>,
        id: Graphid,
        properties: &[String],
        value: serde_json::Value,
    ) -> ExecutionResult<()> {
        // Get current vertex
        let mut vertex = tx
            .get_vertex(id)
            .await?
            .ok_or_else(|| ExecutionError::InvalidExpression("Vertex not found".to_string()))?;

        // Update property
        self.set_nested_property(&mut vertex.properties, properties, value)?;

        // Save back
        tx.update_vertex(id, vertex.properties).await?;

        Ok(())
    }

    async fn update_edge_property(
        &self,
        tx: &mut Box<dyn crate::storage::GraphTransaction>,
        id: Graphid,
        properties: &[String],
        value: serde_json::Value,
    ) -> ExecutionResult<()> {
        // Get current edge
        let mut edge = tx
            .get_edge(id)
            .await?
            .ok_or_else(|| ExecutionError::InvalidExpression("Edge not found".to_string()))?;

        // Update property
        self.set_nested_property(&mut edge.properties, properties, value)?;

        // Save back
        tx.update_edge(id, edge.properties).await?;

        Ok(())
    }

    fn set_nested_property(
        &self,
        target: &mut serde_json::Value,
        properties: &[String],
        value: serde_json::Value,
    ) -> ExecutionResult<()> {
        if properties.is_empty() {
            return Err(ExecutionError::InvalidExpression(
                "Empty property path".to_string(),
            ));
        }

        if properties.len() == 1 {
            // Simple case: set top-level property
            if let serde_json::Value::Object(map) = target {
                map.insert(properties[0].clone(), value);
                Ok(())
            } else {
                Err(ExecutionError::TypeMismatch {
                    expected: "Object".to_string(),
                    actual: format!("{:?}", target),
                })
            }
        } else {
            // Nested case: traverse to parent
            let parent_path = &properties[..properties.len() - 1];
            let final_key = &properties[properties.len() - 1];

            let mut current = target;
            for key in parent_path {
                current = current
                    .get_mut(key)
                    .ok_or_else(|| ExecutionError::PropertyNotFound(key.clone()))?;
            }

            if let serde_json::Value::Object(map) = current {
                map.insert(final_key.clone(), value);
                Ok(())
            } else {
                Err(ExecutionError::TypeMismatch {
                    expected: "Object".to_string(),
                    actual: format!("{:?}", current),
                })
            }
        }
    }

    fn evaluate_expression(
        &self,
        expr: &Expression,
        row: &Row,
    ) -> ExecutionResult<serde_json::Value> {
        match expr {
            Expression::Literal(lit) => self.literal_to_json(lit),
            Expression::Variable(var) => {
                let value = row
                    .get(var)
                    .ok_or_else(|| ExecutionError::VariableNotFound(var.clone()))?;
                self.value_to_json(value)
            }
            Expression::Property(prop) => {
                let entity = row
                    .get(&prop.base)
                    .ok_or_else(|| ExecutionError::VariableNotFound(prop.base.clone()))?;

                match entity {
                    Value::Vertex(v) => {
                        self.extract_property(&v.properties, &prop.properties)
                    }
                    Value::Edge(e) => {
                        self.extract_property(&e.properties, &prop.properties)
                    }
                    _ => Err(ExecutionError::TypeMismatch {
                        expected: "Vertex or Edge".to_string(),
                        actual: format!("{:?}", entity),
                    }),
                }
            }
            Expression::BinaryOp { left, op, right } => {
                let left_val = self.evaluate_expression(left, row)?;
                let right_val = self.evaluate_expression(right, row)?;
                self.apply_binary_op(&left_val, op, &right_val)
            }
            _ => Err(ExecutionError::UnsupportedOperation(
                "Complex expressions not yet supported in SET".to_string(),
            )),
        }
    }

    fn extract_property(
        &self,
        json: &serde_json::Value,
        properties: &[String],
    ) -> ExecutionResult<serde_json::Value> {
        let mut current = json;

        for prop in properties {
            current = current
                .get(prop)
                .ok_or_else(|| ExecutionError::PropertyNotFound(prop.clone()))?;
        }

        Ok(current.clone())
    }

    fn apply_binary_op(
        &self,
        left: &serde_json::Value,
        op: &BinaryOperator,
        right: &serde_json::Value,
    ) -> ExecutionResult<serde_json::Value> {
        match op {
            BinaryOperator::Add => {
                // Handle numeric addition and string concatenation
                match (left, right) {
                    (serde_json::Value::Number(l), serde_json::Value::Number(r)) => {
                        if let (Some(li), Some(ri)) = (l.as_i64(), r.as_i64()) {
                            Ok(serde_json::json!(li + ri))
                        } else if let (Some(lf), Some(rf)) = (l.as_f64(), r.as_f64()) {
                            Ok(serde_json::json!(lf + rf))
                        } else {
                            Err(ExecutionError::TypeMismatch {
                                expected: "Number".to_string(),
                                actual: format!("{:?} + {:?}", left, right),
                            })
                        }
                    }
                    (serde_json::Value::String(l), serde_json::Value::String(r)) => {
                        Ok(serde_json::json!(format!("{}{}", l, r)))
                    }
                    _ => Err(ExecutionError::TypeMismatch {
                        expected: "Number or String".to_string(),
                        actual: format!("{:?} + {:?}", left, right),
                    }),
                }
            }
            BinaryOperator::Subtract | BinaryOperator::Multiply | BinaryOperator::Divide => {
                match (left, right) {
                    (serde_json::Value::Number(l), serde_json::Value::Number(r)) => {
                        if let (Some(li), Some(ri)) = (l.as_i64(), r.as_i64()) {
                            let result = match op {
                                BinaryOperator::Subtract => li - ri,
                                BinaryOperator::Multiply => li * ri,
                                BinaryOperator::Divide => {
                                    if ri == 0 {
                                        return Err(ExecutionError::InvalidExpression(
                                            "Division by zero".to_string(),
                                        ));
                                    }
                                    li / ri
                                }
                                _ => unreachable!(),
                            };
                            Ok(serde_json::json!(result))
                        } else if let (Some(lf), Some(rf)) = (l.as_f64(), r.as_f64()) {
                            let result = match op {
                                BinaryOperator::Subtract => lf - rf,
                                BinaryOperator::Multiply => lf * rf,
                                BinaryOperator::Divide => lf / rf,
                                _ => unreachable!(),
                            };
                            Ok(serde_json::json!(result))
                        } else {
                            Err(ExecutionError::TypeMismatch {
                                expected: "Number".to_string(),
                                actual: format!("{:?}", left),
                            })
                        }
                    }
                    _ => Err(ExecutionError::TypeMismatch {
                        expected: "Number".to_string(),
                        actual: format!("{:?}", left),
                    }),
                }
            }
            _ => Err(ExecutionError::UnsupportedOperation(format!(
                "Operator {:?} not supported in SET",
                op
            ))),
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
                    .map(|e| self.evaluate_expression(e, &Row::new()))
                    .collect();
                Ok(serde_json::Value::Array(values?))
            }
            Literal::Map(map) => {
                let obj: ExecutionResult<serde_json::Map<String, serde_json::Value>> = map
                    .iter()
                    .map(|(k, v)| Ok((k.clone(), self.evaluate_expression(v, &Row::new())?)))
                    .collect();
                Ok(serde_json::Value::Object(obj?))
            }
        }
    }

    fn value_to_json(&self, value: &Value) -> ExecutionResult<serde_json::Value> {
        match value {
            Value::Null => Ok(serde_json::Value::Null),
            Value::Boolean(b) => Ok(serde_json::Value::Bool(*b)),
            Value::Integer(i) => Ok(serde_json::json!(i)),
            Value::Float(f) => Ok(serde_json::json!(f)),
            Value::String(s) => Ok(serde_json::Value::String(s.clone())),
            Value::List(items) => {
                let arr: Result<Vec<_>, _> = items.iter().map(|v| self.value_to_json(v)).collect();
                Ok(serde_json::Value::Array(arr?))
            }
            Value::Map(map) => {
                let obj: Result<serde_json::Map<String, serde_json::Value>, ExecutionError> = map
                    .iter()
                    .map(|(k, v)| Ok((k.clone(), self.value_to_json(v)?)))
                    .collect();
                Ok(serde_json::Value::Object(obj?))
            }
            Value::Vertex(v) => Ok(v.properties.clone()),
            Value::Edge(e) => Ok(e.properties.clone()),
            Value::Path(_) => Err(ExecutionError::TypeMismatch {
                expected: "Simple value".to_string(),
                actual: "Path".to_string(),
            }),
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
    async fn test_set_property() {
        let (storage, _temp) = setup_test_storage().await;

        // Create vertex
        let mut tx = storage.begin_transaction().await.unwrap();
        let vertex = tx
            .create_vertex("Person", serde_json::json!({"name": "Alice", "age": 30}))
            .await
            .unwrap();
        tx.commit().await.unwrap();

        // SET n.age = 31
        let mut executor = SetExecutor::new(storage.clone());
        let row = Row::new().with_binding("n".to_string(), Value::Vertex(vertex.clone()));

        let set_item = SetItem {
            property: PropertyExpression {
                base: "n".to_string(),
                properties: vec!["age".to_string()],
            },
            value: Expression::Literal(Literal::Integer(31)),
        };

        executor
            .execute_with_context(&[set_item], &[row])
            .await
            .unwrap();

        // Verify update
        let updated = storage.get_vertex(vertex.id).await.unwrap().unwrap();
        assert_eq!(updated.properties["age"], 31);
        assert_eq!(updated.properties["name"], "Alice");
    }

    #[tokio::test]
    async fn test_set_with_expression() {
        let (storage, _temp) = setup_test_storage().await;

        // Create vertex
        let mut tx = storage.begin_transaction().await.unwrap();
        let vertex = tx
            .create_vertex("Person", serde_json::json!({"name": "Alice", "age": 30}))
            .await
            .unwrap();
        tx.commit().await.unwrap();

        // SET n.age = n.age + 1
        let mut executor = SetExecutor::new(storage.clone());
        let row = Row::new().with_binding("n".to_string(), Value::Vertex(vertex.clone()));

        let set_item = SetItem {
            property: PropertyExpression {
                base: "n".to_string(),
                properties: vec!["age".to_string()],
            },
            value: Expression::BinaryOp {
                left: Box::new(Expression::Property(PropertyExpression {
                    base: "n".to_string(),
                    properties: vec!["age".to_string()],
                })),
                op: BinaryOperator::Add,
                right: Box::new(Expression::Literal(Literal::Integer(1))),
            },
        };

        executor
            .execute_with_context(&[set_item], &[row])
            .await
            .unwrap();

        // Verify update
        let updated = storage.get_vertex(vertex.id).await.unwrap().unwrap();
        assert_eq!(updated.properties["age"], 31);
    }
}
