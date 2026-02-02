/// Query executor module
///
/// This module implements execution engines for Cypher queries.

mod match_executor;
mod create_executor;
mod delete_executor;
mod set_executor;

pub use match_executor::MatchExecutor;
pub use create_executor::CreateExecutor;
pub use delete_executor::DeleteExecutor;
pub use set_executor::SetExecutor;

use crate::parser::ast::*;
use crate::storage::GraphStorage;
use crate::types::{Edge, Graphid, Vertex};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

/// Execution errors
#[derive(Error, Debug)]
pub enum ExecutionError {
    #[error("Storage error: {0}")]
    StorageError(#[from] crate::storage::StorageError),

    #[error("Variable not found: {0}")]
    VariableNotFound(String),

    #[error("Type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },

    #[error("Invalid expression: {0}")]
    InvalidExpression(String),

    #[error("Property not found: {0}")]
    PropertyNotFound(String),

    #[error("Unsupported operation: {0}")]
    UnsupportedOperation(String),
}

pub type ExecutionResult<T> = Result<T, ExecutionError>;

/// A value in the execution context
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum Value {
    #[default]
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    List(Vec<Value>),
    Map(HashMap<String, Value>),
    Vertex(Vertex),
    Edge(Edge),
    Path(Vec<Value>), // Alternating vertices and edges
}

impl Value {
    pub fn as_vertex(&self) -> ExecutionResult<&Vertex> {
        match self {
            Value::Vertex(v) => Ok(v),
            _ => Err(ExecutionError::TypeMismatch {
                expected: "Vertex".to_string(),
                actual: format!("{:?}", self),
            }),
        }
    }

    pub fn as_graphid(&self) -> ExecutionResult<Graphid> {
        match self {
            Value::Vertex(v) => Ok(v.id),
            Value::Edge(e) => Ok(e.id),
            _ => Err(ExecutionError::TypeMismatch {
                expected: "Vertex or Edge".to_string(),
                actual: format!("{:?}", self),
            }),
        }
    }

    pub fn as_i64(&self) -> ExecutionResult<i64> {
        match self {
            Value::Integer(i) => Ok(*i),
            _ => Err(ExecutionError::TypeMismatch {
                expected: "Integer".to_string(),
                actual: format!("{:?}", self),
            }),
        }
    }

    pub fn as_string(&self) -> ExecutionResult<&str> {
        match self {
            Value::String(s) => Ok(s),
            _ => Err(ExecutionError::TypeMismatch {
                expected: "String".to_string(),
                actual: format!("{:?}", self),
            }),
        }
    }

    pub fn as_bool(&self) -> ExecutionResult<bool> {
        match self {
            Value::Boolean(b) => Ok(*b),
            _ => Err(ExecutionError::TypeMismatch {
                expected: "Boolean".to_string(),
                actual: format!("{:?}", self),
            }),
        }
    }
}

/// A row of values (result of pattern matching)
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Row {
    pub bindings: HashMap<String, Value>,
}

impl Row {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_binding(mut self, name: impl Into<String>, value: Value) -> Self {
        self.bindings.insert(name.into(), value);
        self
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.bindings.get(name)
    }

    pub fn insert(&mut self, name: impl Into<String>, value: Value) {
        self.bindings.insert(name.into(), value);
    }
}

/// Query executor
pub struct QueryExecutor {
    storage: Arc<dyn GraphStorage>,
}

impl QueryExecutor {
    pub fn new(storage: Arc<dyn GraphStorage>) -> Self {
        Self { storage }
    }

    /// Execute a Cypher query
    pub async fn execute(&self, query: CypherQuery) -> ExecutionResult<Vec<Row>> {
        match query {
            CypherQuery::Read {
                match_clause,
                where_clause,
                return_clause,
            } => {
                let match_executor = MatchExecutor::new(self.storage.clone());
                let mut rows = match_executor
                    .execute(&match_clause, where_clause.as_ref())
                    .await?;

                // Apply RETURN projection
                self.apply_return(&mut rows, &return_clause)?;

                Ok(rows)
            }

            CypherQuery::Write(write_clause) => {
                match write_clause {
                    WriteClause::Create { patterns } => {
                        let mut create_executor = CreateExecutor::new(self.storage.clone());
                        create_executor.execute(&patterns).await
                    }
                    WriteClause::Delete {
                        expressions,
                        detach,
                    } => {
                        // First, we need to evaluate expressions in a MATCH context
                        // For now, assume expressions are simple variables
                        let mut delete_executor = DeleteExecutor::new(self.storage.clone());
                        delete_executor.execute(&expressions, detach).await?;
                        Ok(vec![])
                    }
                    WriteClause::Set { items } => {
                        let mut set_executor = SetExecutor::new(self.storage.clone());
                        set_executor.execute(&items).await?;
                        Ok(vec![])
                    }
                }
            }

            CypherQuery::Mixed {
                match_clause,
                where_clause,
                write_clause,
                return_clause,
            } => {
                // First execute MATCH
                let match_executor = MatchExecutor::new(self.storage.clone());
                let rows = match_executor
                    .execute(&match_clause, where_clause.as_ref())
                    .await?;

                // Then execute write operation on matched rows
                match write_clause {
                    WriteClause::Set { items } => {
                        let mut set_executor = SetExecutor::new(self.storage.clone());
                        set_executor.execute_with_context(&items, &rows).await?;
                    }
                    WriteClause::Delete {
                        expressions,
                        detach,
                    } => {
                        let mut delete_executor = DeleteExecutor::new(self.storage.clone());
                        delete_executor
                            .execute_with_context(&expressions, detach, &rows)
                            .await?;
                    }
                    WriteClause::Create { patterns } => {
                        let mut create_executor = CreateExecutor::new(self.storage.clone());
                        create_executor.execute(&patterns).await?;
                    }
                }

                // Return results if specified
                if let Some(return_clause) = return_clause {
                    let mut result_rows = rows;
                    self.apply_return(&mut result_rows, &return_clause)?;
                    Ok(result_rows)
                } else {
                    Ok(vec![])
                }
            }
        }
    }

    fn apply_return(
        &self,
        rows: &mut Vec<Row>,
        return_clause: &ReturnClause,
    ) -> ExecutionResult<()> {
        // Step 1: Apply projection (filter bindings)
        for row in rows.iter_mut() {
            let mut new_bindings = HashMap::new();

            for item in &return_clause.items {
                match &item.expression {
                    Expression::Variable(var) => {
                        if let Some(value) = row.get(var) {
                            let key = item.alias.clone().unwrap_or_else(|| var.clone());
                            new_bindings.insert(key, value.clone());
                        }
                    }
                    Expression::Property(prop) => {
                        // Handle property access like n.name
                        if let Some(value) = row.get(&prop.base) {
                            let prop_value = self.get_property(value, &prop.properties)?;
                            let key = item
                                .alias
                                .clone()
                                .unwrap_or_else(|| format!("{}.{}", prop.base, prop.properties.join(".")));
                            new_bindings.insert(key, prop_value);
                        }
                    }
                    _ => {
                        // TODO: Evaluate complex expressions
                        return Err(ExecutionError::UnsupportedOperation(
                            "Complex expressions in RETURN".to_string(),
                        ));
                    }
                }
            }

            row.bindings = new_bindings;
        }

        // Step 2: Apply ORDER BY
        if let Some(sort_items) = &return_clause.order_by {
            self.apply_order_by(rows, sort_items)?;
        }

        // Step 3: Apply LIMIT
        if let Some(limit) = return_clause.limit {
            if limit >= 0 {
                rows.truncate(limit as usize);
            }
        }

        Ok(())
    }

    /// Apply ORDER BY sorting to rows
    fn apply_order_by(
        &self,
        rows: &mut Vec<Row>,
        sort_items: &[SortItem],
    ) -> ExecutionResult<()> {
        if sort_items.is_empty() {
            return Ok(());
        }

        // Sort rows using the sort items
        rows.sort_by(|a, b| {
            for sort_item in sort_items {
                let val_a = self.evaluate_sort_expression(&sort_item.expression, a);
                let val_b = self.evaluate_sort_expression(&sort_item.expression, b);

                let ordering = match (val_a, val_b) {
                    (Ok(ref va), Ok(ref vb)) => self.compare_values(va, vb),
                    (Err(_), Ok(_)) => std::cmp::Ordering::Greater, // Errors go last
                    (Ok(_), Err(_)) => std::cmp::Ordering::Less,
                    (Err(_), Err(_)) => std::cmp::Ordering::Equal,
                };

                let ordering = if sort_item.ascending {
                    ordering
                } else {
                    ordering.reverse()
                };

                if ordering != std::cmp::Ordering::Equal {
                    return ordering;
                }
            }
            std::cmp::Ordering::Equal
        });

        Ok(())
    }

    /// Evaluate an expression for sorting purposes
    fn evaluate_sort_expression(&self, expr: &Expression, row: &Row) -> ExecutionResult<Value> {
        match expr {
            Expression::Variable(var) => {
                row.get(var)
                    .cloned()
                    .ok_or_else(|| ExecutionError::VariableNotFound(var.clone()))
            }
            Expression::Property(prop) => {
                if let Some(value) = row.get(&prop.base) {
                    self.get_property(value, &prop.properties)
                } else {
                    Err(ExecutionError::VariableNotFound(prop.base.clone()))
                }
            }
            Expression::Literal(lit) => Ok(self.literal_to_value(lit)),
            _ => Err(ExecutionError::UnsupportedOperation(
                "Complex expressions in ORDER BY".to_string(),
            )),
        }
    }

    /// Convert a Literal to a Value
    fn literal_to_value(&self, lit: &Literal) -> Value {
        match lit {
            Literal::Null => Value::Null,
            Literal::Boolean(b) => Value::Boolean(*b),
            Literal::Integer(i) => Value::Integer(*i),
            Literal::Float(f) => Value::Float(*f),
            Literal::String(s) => Value::String(s.clone()),
            Literal::List(items) => {
                Value::List(
                    items.iter()
                        .filter_map(|e| match e {
                            Expression::Literal(lit) => Some(self.literal_to_value(lit)),
                            _ => None,
                        })
                        .collect()
                )
            }
            Literal::Map(map) => {
                Value::Map(
                    map.iter()
                        .filter_map(|(k, v)| match v {
                            Expression::Literal(lit) => Some((k.clone(), self.literal_to_value(lit))),
                            _ => None,
                        })
                        .collect()
                )
            }
        }
    }

    /// Compare two values for sorting
    fn compare_values(&self, left: &Value, right: &Value) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        match (left, right) {
            // Null handling: NULL is always last
            (Value::Null, Value::Null) => Ordering::Equal,
            (Value::Null, _) => Ordering::Greater,
            (_, Value::Null) => Ordering::Less,

            // Integer comparison
            (Value::Integer(a), Value::Integer(b)) => a.cmp(b),

            // Float comparison
            (Value::Float(a), Value::Float(b)) => {
                a.partial_cmp(b).unwrap_or(Ordering::Equal)
            }

            // Mixed numeric comparison
            (Value::Integer(a), Value::Float(b)) => {
                (*a as f64).partial_cmp(b).unwrap_or(Ordering::Equal)
            }
            (Value::Float(a), Value::Integer(b)) => {
                a.partial_cmp(&(*b as f64)).unwrap_or(Ordering::Equal)
            }

            // String comparison
            (Value::String(a), Value::String(b)) => a.cmp(b),

            // Boolean comparison (false < true)
            (Value::Boolean(a), Value::Boolean(b)) => a.cmp(b),

            // Different types: compare by type name
            _ => format!("{:?}", left).cmp(&format!("{:?}", right)),
        }
    }

    fn get_property(&self, value: &Value, properties: &[String]) -> ExecutionResult<Value> {
        match value {
            Value::Vertex(v) => {
                let json = &v.properties;
                self.extract_json_property(json, properties)
            }
            Value::Edge(e) => {
                let json = &e.properties;
                self.extract_json_property(json, properties)
            }
            _ => Err(ExecutionError::TypeMismatch {
                expected: "Vertex or Edge".to_string(),
                actual: format!("{:?}", value),
            }),
        }
    }

    fn extract_json_property(
        &self,
        json: &serde_json::Value,
        properties: &[String],
    ) -> ExecutionResult<Value> {
        let mut current = json;

        for prop in properties {
            current = current
                .get(prop)
                .ok_or_else(|| ExecutionError::PropertyNotFound(prop.clone()))?;
        }

        Ok(self.json_to_value(current))
    }

    fn json_to_value(&self, json: &serde_json::Value) -> Value {
        match json {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(b) => Value::Boolean(*b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Value::Integer(i)
                } else if let Some(f) = n.as_f64() {
                    Value::Float(f)
                } else {
                    Value::Null
                }
            }
            serde_json::Value::String(s) => Value::String(s.clone()),
            serde_json::Value::Array(arr) => {
                Value::List(arr.iter().map(|v| self.json_to_value(v)).collect())
            }
            serde_json::Value::Object(obj) => Value::Map(
                obj.iter()
                    .map(|(k, v)| (k.clone(), self.json_to_value(v)))
                    .collect(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::rocksdb_store::RocksDbStorage;
    use tempfile::TempDir;

    #[test]
    fn test_value_conversions() {
        let v = Value::Integer(42);
        assert_eq!(v.as_i64().unwrap(), 42);

        let v = Value::String("hello".to_string());
        assert_eq!(v.as_string().unwrap(), "hello");

        let v = Value::Boolean(true);
        assert!(v.as_bool().unwrap());
    }

    #[test]
    fn test_row_operations() {
        let row = Row::new()
            .with_binding("x".to_string(), Value::Integer(42))
            .with_binding("y".to_string(), Value::String("hello".to_string()));

        assert_eq!(row.get("x"), Some(&Value::Integer(42)));
        assert_eq!(row.get("y"), Some(&Value::String("hello".to_string())));
        assert_eq!(row.get("z"), None);
    }

    fn create_test_executor() -> (QueryExecutor, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage: Arc<dyn GraphStorage> = Arc::new(
            RocksDbStorage::new(temp_dir.path(), "test_graph").unwrap()
        );
        (QueryExecutor::new(storage), temp_dir)
    }

    #[test]
    fn test_order_by_integer_asc() {
        let (executor, _temp) = create_test_executor();

        let mut rows = vec![
            Row::new().with_binding("age", Value::Integer(30)),
            Row::new().with_binding("age", Value::Integer(25)),
            Row::new().with_binding("age", Value::Integer(35)),
        ];

        let sort_items = vec![SortItem {
            expression: Expression::Variable("age".to_string()),
            ascending: true,
        }];

        executor.apply_order_by(&mut rows, &sort_items).unwrap();

        assert_eq!(rows[0].get("age"), Some(&Value::Integer(25)));
        assert_eq!(rows[1].get("age"), Some(&Value::Integer(30)));
        assert_eq!(rows[2].get("age"), Some(&Value::Integer(35)));
    }

    #[test]
    fn test_order_by_integer_desc() {
        let (executor, _temp) = create_test_executor();

        let mut rows = vec![
            Row::new().with_binding("age", Value::Integer(30)),
            Row::new().with_binding("age", Value::Integer(25)),
            Row::new().with_binding("age", Value::Integer(35)),
        ];

        let sort_items = vec![SortItem {
            expression: Expression::Variable("age".to_string()),
            ascending: false,
        }];

        executor.apply_order_by(&mut rows, &sort_items).unwrap();

        assert_eq!(rows[0].get("age"), Some(&Value::Integer(35)));
        assert_eq!(rows[1].get("age"), Some(&Value::Integer(30)));
        assert_eq!(rows[2].get("age"), Some(&Value::Integer(25)));
    }

    #[test]
    fn test_order_by_string() {
        let (executor, _temp) = create_test_executor();

        let mut rows = vec![
            Row::new().with_binding("name", Value::String("Charlie".to_string())),
            Row::new().with_binding("name", Value::String("Alice".to_string())),
            Row::new().with_binding("name", Value::String("Bob".to_string())),
        ];

        let sort_items = vec![SortItem {
            expression: Expression::Variable("name".to_string()),
            ascending: true,
        }];

        executor.apply_order_by(&mut rows, &sort_items).unwrap();

        assert_eq!(rows[0].get("name"), Some(&Value::String("Alice".to_string())));
        assert_eq!(rows[1].get("name"), Some(&Value::String("Bob".to_string())));
        assert_eq!(rows[2].get("name"), Some(&Value::String("Charlie".to_string())));
    }

    #[test]
    fn test_order_by_with_nulls() {
        let (executor, _temp) = create_test_executor();

        let mut rows = vec![
            Row::new().with_binding("age", Value::Integer(30)),
            Row::new().with_binding("age", Value::Null),
            Row::new().with_binding("age", Value::Integer(25)),
        ];

        let sort_items = vec![SortItem {
            expression: Expression::Variable("age".to_string()),
            ascending: true,
        }];

        executor.apply_order_by(&mut rows, &sort_items).unwrap();

        // NULL should be last
        assert_eq!(rows[0].get("age"), Some(&Value::Integer(25)));
        assert_eq!(rows[1].get("age"), Some(&Value::Integer(30)));
        assert_eq!(rows[2].get("age"), Some(&Value::Null));
    }

    #[test]
    fn test_order_by_multiple_columns() {
        let (executor, _temp) = create_test_executor();

        let mut rows = vec![
            Row::new()
                .with_binding("city", Value::String("Beijing".to_string()))
                .with_binding("age", Value::Integer(30)),
            Row::new()
                .with_binding("city", Value::String("Beijing".to_string()))
                .with_binding("age", Value::Integer(25)),
            Row::new()
                .with_binding("city", Value::String("Shanghai".to_string()))
                .with_binding("age", Value::Integer(28)),
        ];

        // ORDER BY city ASC, age ASC
        let sort_items = vec![
            SortItem {
                expression: Expression::Variable("city".to_string()),
                ascending: true,
            },
            SortItem {
                expression: Expression::Variable("age".to_string()),
                ascending: true,
            },
        ];

        executor.apply_order_by(&mut rows, &sort_items).unwrap();

        // Beijing should come first, then sorted by age
        assert_eq!(rows[0].get("city"), Some(&Value::String("Beijing".to_string())));
        assert_eq!(rows[0].get("age"), Some(&Value::Integer(25)));
        assert_eq!(rows[1].get("city"), Some(&Value::String("Beijing".to_string())));
        assert_eq!(rows[1].get("age"), Some(&Value::Integer(30)));
        assert_eq!(rows[2].get("city"), Some(&Value::String("Shanghai".to_string())));
    }

    #[test]
    fn test_limit() {
        let (executor, _temp) = create_test_executor();

        let mut rows = vec![
            Row::new().with_binding("n", Value::Integer(1)),
            Row::new().with_binding("n", Value::Integer(2)),
            Row::new().with_binding("n", Value::Integer(3)),
            Row::new().with_binding("n", Value::Integer(4)),
            Row::new().with_binding("n", Value::Integer(5)),
        ];

        let return_clause = ReturnClause {
            items: vec![ReturnItem {
                expression: Expression::Variable("n".to_string()),
                alias: None,
            }],
            order_by: None,
            limit: Some(3),
        };

        executor.apply_return(&mut rows, &return_clause).unwrap();

        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].get("n"), Some(&Value::Integer(1)));
        assert_eq!(rows[1].get("n"), Some(&Value::Integer(2)));
        assert_eq!(rows[2].get("n"), Some(&Value::Integer(3)));
    }

    #[test]
    fn test_order_by_and_limit() {
        let (executor, _temp) = create_test_executor();

        let mut rows = vec![
            Row::new().with_binding("age", Value::Integer(30)),
            Row::new().with_binding("age", Value::Integer(25)),
            Row::new().with_binding("age", Value::Integer(35)),
            Row::new().with_binding("age", Value::Integer(20)),
            Row::new().with_binding("age", Value::Integer(40)),
        ];

        // ORDER BY age DESC LIMIT 3
        let return_clause = ReturnClause {
            items: vec![ReturnItem {
                expression: Expression::Variable("age".to_string()),
                alias: None,
            }],
            order_by: Some(vec![SortItem {
                expression: Expression::Variable("age".to_string()),
                ascending: false,
            }]),
            limit: Some(3),
        };

        executor.apply_return(&mut rows, &return_clause).unwrap();

        assert_eq!(rows.len(), 3);
        // Should be top 3 oldest: 40, 35, 30
        assert_eq!(rows[0].get("age"), Some(&Value::Integer(40)));
        assert_eq!(rows[1].get("age"), Some(&Value::Integer(35)));
        assert_eq!(rows[2].get("age"), Some(&Value::Integer(30)));
    }
}
