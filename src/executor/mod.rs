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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
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
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub bindings: HashMap<String, Value>,
}

impl Row {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    pub fn with_binding(mut self, name: String, value: Value) -> Self {
        self.bindings.insert(name, value);
        self
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.bindings.get(name)
    }

    pub fn insert(&mut self, name: String, value: Value) {
        self.bindings.insert(name, value);
    }
}

impl Default for Row {
    fn default() -> Self {
        Self::new()
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
        // For now, simple implementation: just filter bindings
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

        Ok(())
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
}
