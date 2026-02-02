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
        // Check if any return item contains an aggregate function
        let has_aggregates = return_clause.items.iter().any(|item| {
            self.contains_aggregate(&item.expression)
        });

        if has_aggregates {
            // Aggregate mode: compute aggregates over all rows
            let aggregated_row = self.compute_aggregates(rows, &return_clause.items)?;
            rows.clear();
            rows.push(aggregated_row);
        } else {
            // Normal mode: Apply projection (filter bindings)
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

            // Apply ORDER BY (only for non-aggregate queries)
            if let Some(sort_items) = &return_clause.order_by {
                self.apply_order_by(rows, sort_items)?;
            }

            // Apply LIMIT
            if let Some(limit) = return_clause.limit {
                if limit >= 0 {
                    rows.truncate(limit as usize);
                }
            }
        }

        Ok(())
    }

    /// Check if an expression contains an aggregate function
    fn contains_aggregate(&self, expr: &Expression) -> bool {
        match expr {
            Expression::FunctionCall { name, .. } => {
                let name_upper = name.to_uppercase();
                matches!(name_upper.as_str(), "COUNT" | "SUM" | "AVG" | "MIN" | "MAX")
            }
            Expression::BinaryOp { left, right, .. } => {
                self.contains_aggregate(left) || self.contains_aggregate(right)
            }
            Expression::UnaryOp { expr, .. } => self.contains_aggregate(expr),
            _ => false,
        }
    }

    /// Compute aggregates over all rows
    fn compute_aggregates(
        &self,
        rows: &[Row],
        items: &[ReturnItem],
    ) -> ExecutionResult<Row> {
        let mut result = Row::new();

        for item in items {
            let key = self.get_return_key(item);
            let value = self.evaluate_aggregate(&item.expression, rows)?;
            result.insert(key, value);
        }

        Ok(result)
    }

    /// Get the key name for a return item
    fn get_return_key(&self, item: &ReturnItem) -> String {
        if let Some(alias) = &item.alias {
            alias.clone()
        } else {
            match &item.expression {
                Expression::Variable(var) => var.clone(),
                Expression::Property(prop) => {
                    format!("{}.{}", prop.base, prop.properties.join("."))
                }
                Expression::FunctionCall { name, args } => {
                    if args.is_empty() {
                        format!("{}()", name)
                    } else {
                        format!("{}(...)", name)
                    }
                }
                _ => "expr".to_string(),
            }
        }
    }

    /// Evaluate an aggregate expression
    fn evaluate_aggregate(&self, expr: &Expression, rows: &[Row]) -> ExecutionResult<Value> {
        match expr {
            Expression::FunctionCall { name, args } => {
                let name_upper = name.to_uppercase();
                match name_upper.as_str() {
                    "COUNT" => self.aggregate_count(args, rows),
                    "SUM" => self.aggregate_sum(args, rows),
                    "AVG" => self.aggregate_avg(args, rows),
                    "MIN" => self.aggregate_min(args, rows),
                    "MAX" => self.aggregate_max(args, rows),
                    _ => Err(ExecutionError::UnsupportedOperation(
                        format!("Unknown aggregate function: {}", name),
                    )),
                }
            }
            Expression::Variable(var) => {
                // Non-aggregate in aggregate context: return first row's value
                rows.first()
                    .and_then(|row| row.get(var).cloned())
                    .ok_or_else(|| ExecutionError::VariableNotFound(var.clone()))
            }
            Expression::Property(prop) => {
                // Non-aggregate property: return first row's value
                if let Some(row) = rows.first() {
                    if let Some(value) = row.get(&prop.base) {
                        return self.get_property(value, &prop.properties);
                    }
                }
                Err(ExecutionError::VariableNotFound(prop.base.clone()))
            }
            _ => Err(ExecutionError::UnsupportedOperation(
                "Unsupported expression in aggregate context".to_string(),
            )),
        }
    }

    /// COUNT aggregate function
    fn aggregate_count(&self, args: &[Expression], rows: &[Row]) -> ExecutionResult<Value> {
        if args.is_empty() {
            // COUNT(*) - count all rows
            return Ok(Value::Integer(rows.len() as i64));
        }

        // COUNT(expr) - count non-null values
        let arg = &args[0];
        let count = rows.iter().filter(|row| {
            match self.evaluate_row_expression(arg, row) {
                Ok(Value::Null) => false,
                Ok(_) => true,
                Err(_) => false,
            }
        }).count();

        Ok(Value::Integer(count as i64))
    }

    /// SUM aggregate function
    fn aggregate_sum(&self, args: &[Expression], rows: &[Row]) -> ExecutionResult<Value> {
        if args.is_empty() {
            return Err(ExecutionError::InvalidExpression(
                "SUM requires an argument".to_string(),
            ));
        }

        let arg = &args[0];
        let mut sum_int: i64 = 0;
        let mut sum_float: f64 = 0.0;
        let mut has_float = false;
        let mut count = 0;

        for row in rows {
            match self.evaluate_row_expression(arg, row)? {
                Value::Integer(i) => {
                    sum_int += i;
                    sum_float += i as f64;
                    count += 1;
                }
                Value::Float(f) => {
                    sum_float += f;
                    has_float = true;
                    count += 1;
                }
                Value::Null => {} // Skip nulls
                _ => {
                    return Err(ExecutionError::TypeMismatch {
                        expected: "Number".to_string(),
                        actual: "Non-numeric value".to_string(),
                    });
                }
            }
        }

        if count == 0 {
            return Ok(Value::Null);
        }

        if has_float {
            Ok(Value::Float(sum_float))
        } else {
            Ok(Value::Integer(sum_int))
        }
    }

    /// AVG aggregate function
    fn aggregate_avg(&self, args: &[Expression], rows: &[Row]) -> ExecutionResult<Value> {
        if args.is_empty() {
            return Err(ExecutionError::InvalidExpression(
                "AVG requires an argument".to_string(),
            ));
        }

        let arg = &args[0];
        let mut sum: f64 = 0.0;
        let mut count = 0;

        for row in rows {
            match self.evaluate_row_expression(arg, row)? {
                Value::Integer(i) => {
                    sum += i as f64;
                    count += 1;
                }
                Value::Float(f) => {
                    sum += f;
                    count += 1;
                }
                Value::Null => {} // Skip nulls
                _ => {
                    return Err(ExecutionError::TypeMismatch {
                        expected: "Number".to_string(),
                        actual: "Non-numeric value".to_string(),
                    });
                }
            }
        }

        if count == 0 {
            return Ok(Value::Null);
        }

        Ok(Value::Float(sum / count as f64))
    }

    /// MIN aggregate function
    fn aggregate_min(&self, args: &[Expression], rows: &[Row]) -> ExecutionResult<Value> {
        if args.is_empty() {
            return Err(ExecutionError::InvalidExpression(
                "MIN requires an argument".to_string(),
            ));
        }

        let arg = &args[0];
        let mut min_value: Option<Value> = None;

        for row in rows {
            let value = self.evaluate_row_expression(arg, row)?;
            if matches!(value, Value::Null) {
                continue;
            }

            min_value = Some(match min_value {
                None => value,
                Some(current) => {
                    if self.compare_values(&value, &current) == std::cmp::Ordering::Less {
                        value
                    } else {
                        current
                    }
                }
            });
        }

        Ok(min_value.unwrap_or(Value::Null))
    }

    /// MAX aggregate function
    fn aggregate_max(&self, args: &[Expression], rows: &[Row]) -> ExecutionResult<Value> {
        if args.is_empty() {
            return Err(ExecutionError::InvalidExpression(
                "MAX requires an argument".to_string(),
            ));
        }

        let arg = &args[0];
        let mut max_value: Option<Value> = None;

        for row in rows {
            let value = self.evaluate_row_expression(arg, row)?;
            if matches!(value, Value::Null) {
                continue;
            }

            max_value = Some(match max_value {
                None => value,
                Some(current) => {
                    if self.compare_values(&value, &current) == std::cmp::Ordering::Greater {
                        value
                    } else {
                        current
                    }
                }
            });
        }

        Ok(max_value.unwrap_or(Value::Null))
    }

    /// Evaluate an expression for a single row
    fn evaluate_row_expression(&self, expr: &Expression, row: &Row) -> ExecutionResult<Value> {
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
                "Complex expressions in aggregates not yet supported".to_string(),
            )),
        }
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

    // ===== Aggregate Function Tests =====

    #[test]
    fn test_aggregate_count_all() {
        let (executor, _temp) = create_test_executor();

        let rows = vec![
            Row::new().with_binding("n", Value::Integer(1)),
            Row::new().with_binding("n", Value::Integer(2)),
            Row::new().with_binding("n", Value::Integer(3)),
        ];

        // COUNT(*)
        let result = executor.aggregate_count(&[], &rows).unwrap();
        assert_eq!(result, Value::Integer(3));
    }

    #[test]
    fn test_aggregate_count_expr() {
        let (executor, _temp) = create_test_executor();

        let rows = vec![
            Row::new().with_binding("n", Value::Integer(1)),
            Row::new().with_binding("n", Value::Null),
            Row::new().with_binding("n", Value::Integer(3)),
        ];

        // COUNT(n) - should skip nulls
        let result = executor.aggregate_count(
            &[Expression::Variable("n".to_string())],
            &rows,
        ).unwrap();
        assert_eq!(result, Value::Integer(2));
    }

    #[test]
    fn test_aggregate_sum_integers() {
        let (executor, _temp) = create_test_executor();

        let rows = vec![
            Row::new().with_binding("n", Value::Integer(10)),
            Row::new().with_binding("n", Value::Integer(20)),
            Row::new().with_binding("n", Value::Integer(30)),
        ];

        let result = executor.aggregate_sum(
            &[Expression::Variable("n".to_string())],
            &rows,
        ).unwrap();
        assert_eq!(result, Value::Integer(60));
    }

    #[test]
    fn test_aggregate_sum_floats() {
        let (executor, _temp) = create_test_executor();

        let rows = vec![
            Row::new().with_binding("n", Value::Float(1.5)),
            Row::new().with_binding("n", Value::Float(2.5)),
            Row::new().with_binding("n", Value::Float(3.0)),
        ];

        let result = executor.aggregate_sum(
            &[Expression::Variable("n".to_string())],
            &rows,
        ).unwrap();
        assert_eq!(result, Value::Float(7.0));
    }

    #[test]
    fn test_aggregate_sum_with_nulls() {
        let (executor, _temp) = create_test_executor();

        let rows = vec![
            Row::new().with_binding("n", Value::Integer(10)),
            Row::new().with_binding("n", Value::Null),
            Row::new().with_binding("n", Value::Integer(30)),
        ];

        let result = executor.aggregate_sum(
            &[Expression::Variable("n".to_string())],
            &rows,
        ).unwrap();
        assert_eq!(result, Value::Integer(40));
    }

    #[test]
    fn test_aggregate_avg() {
        let (executor, _temp) = create_test_executor();

        let rows = vec![
            Row::new().with_binding("n", Value::Integer(10)),
            Row::new().with_binding("n", Value::Integer(20)),
            Row::new().with_binding("n", Value::Integer(30)),
        ];

        let result = executor.aggregate_avg(
            &[Expression::Variable("n".to_string())],
            &rows,
        ).unwrap();
        assert_eq!(result, Value::Float(20.0));
    }

    #[test]
    fn test_aggregate_avg_with_nulls() {
        let (executor, _temp) = create_test_executor();

        let rows = vec![
            Row::new().with_binding("n", Value::Integer(10)),
            Row::new().with_binding("n", Value::Null),
            Row::new().with_binding("n", Value::Integer(20)),
        ];

        // AVG should skip nulls: (10 + 20) / 2 = 15
        let result = executor.aggregate_avg(
            &[Expression::Variable("n".to_string())],
            &rows,
        ).unwrap();
        assert_eq!(result, Value::Float(15.0));
    }

    #[test]
    fn test_aggregate_min_integers() {
        let (executor, _temp) = create_test_executor();

        let rows = vec![
            Row::new().with_binding("n", Value::Integer(30)),
            Row::new().with_binding("n", Value::Integer(10)),
            Row::new().with_binding("n", Value::Integer(20)),
        ];

        let result = executor.aggregate_min(
            &[Expression::Variable("n".to_string())],
            &rows,
        ).unwrap();
        assert_eq!(result, Value::Integer(10));
    }

    #[test]
    fn test_aggregate_min_strings() {
        let (executor, _temp) = create_test_executor();

        let rows = vec![
            Row::new().with_binding("name", Value::String("Charlie".to_string())),
            Row::new().with_binding("name", Value::String("Alice".to_string())),
            Row::new().with_binding("name", Value::String("Bob".to_string())),
        ];

        let result = executor.aggregate_min(
            &[Expression::Variable("name".to_string())],
            &rows,
        ).unwrap();
        assert_eq!(result, Value::String("Alice".to_string()));
    }

    #[test]
    fn test_aggregate_max_integers() {
        let (executor, _temp) = create_test_executor();

        let rows = vec![
            Row::new().with_binding("n", Value::Integer(30)),
            Row::new().with_binding("n", Value::Integer(10)),
            Row::new().with_binding("n", Value::Integer(20)),
        ];

        let result = executor.aggregate_max(
            &[Expression::Variable("n".to_string())],
            &rows,
        ).unwrap();
        assert_eq!(result, Value::Integer(30));
    }

    #[test]
    fn test_aggregate_max_with_nulls() {
        let (executor, _temp) = create_test_executor();

        let rows = vec![
            Row::new().with_binding("n", Value::Integer(10)),
            Row::new().with_binding("n", Value::Null),
            Row::new().with_binding("n", Value::Integer(30)),
        ];

        // MAX should skip nulls
        let result = executor.aggregate_max(
            &[Expression::Variable("n".to_string())],
            &rows,
        ).unwrap();
        assert_eq!(result, Value::Integer(30));
    }

    #[test]
    fn test_aggregate_empty_rows() {
        let (executor, _temp) = create_test_executor();

        let rows: Vec<Row> = vec![];

        // COUNT(*) of empty should be 0
        let result = executor.aggregate_count(&[], &rows).unwrap();
        assert_eq!(result, Value::Integer(0));

        // SUM/AVG/MIN/MAX of empty should be NULL
        let sum = executor.aggregate_sum(
            &[Expression::Variable("n".to_string())],
            &rows,
        ).unwrap();
        assert_eq!(sum, Value::Null);

        let avg = executor.aggregate_avg(
            &[Expression::Variable("n".to_string())],
            &rows,
        ).unwrap();
        assert_eq!(avg, Value::Null);

        let min = executor.aggregate_min(
            &[Expression::Variable("n".to_string())],
            &rows,
        ).unwrap();
        assert_eq!(min, Value::Null);

        let max = executor.aggregate_max(
            &[Expression::Variable("n".to_string())],
            &rows,
        ).unwrap();
        assert_eq!(max, Value::Null);
    }

    #[test]
    fn test_apply_return_with_count() {
        let (executor, _temp) = create_test_executor();

        let mut rows = vec![
            Row::new().with_binding("n", Value::Integer(1)),
            Row::new().with_binding("n", Value::Integer(2)),
            Row::new().with_binding("n", Value::Integer(3)),
        ];

        // RETURN COUNT(*)
        let return_clause = ReturnClause {
            items: vec![ReturnItem {
                expression: Expression::FunctionCall {
                    name: "COUNT".to_string(),
                    args: vec![],
                },
                alias: Some("count".to_string()),
            }],
            order_by: None,
            limit: None,
        };

        executor.apply_return(&mut rows, &return_clause).unwrap();

        // Should have single row with count
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("count"), Some(&Value::Integer(3)));
    }

    #[test]
    fn test_apply_return_with_multiple_aggregates() {
        let (executor, _temp) = create_test_executor();

        let mut rows = vec![
            Row::new().with_binding("age", Value::Integer(20)),
            Row::new().with_binding("age", Value::Integer(30)),
            Row::new().with_binding("age", Value::Integer(40)),
        ];

        // RETURN COUNT(*) AS cnt, SUM(age) AS total, AVG(age) AS average
        let return_clause = ReturnClause {
            items: vec![
                ReturnItem {
                    expression: Expression::FunctionCall {
                        name: "COUNT".to_string(),
                        args: vec![],
                    },
                    alias: Some("cnt".to_string()),
                },
                ReturnItem {
                    expression: Expression::FunctionCall {
                        name: "SUM".to_string(),
                        args: vec![Expression::Variable("age".to_string())],
                    },
                    alias: Some("total".to_string()),
                },
                ReturnItem {
                    expression: Expression::FunctionCall {
                        name: "AVG".to_string(),
                        args: vec![Expression::Variable("age".to_string())],
                    },
                    alias: Some("average".to_string()),
                },
            ],
            order_by: None,
            limit: None,
        };

        executor.apply_return(&mut rows, &return_clause).unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("cnt"), Some(&Value::Integer(3)));
        assert_eq!(rows[0].get("total"), Some(&Value::Integer(90)));
        assert_eq!(rows[0].get("average"), Some(&Value::Float(30.0)));
    }
}
