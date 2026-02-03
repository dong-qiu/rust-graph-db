/// MATCH clause executor
///
/// Implements pattern matching for Cypher MATCH queries.

use super::{ExecutionError, ExecutionResult, Row, Value};
use crate::parser::ast::*;
use crate::storage::GraphStorage;
use crate::types::{Edge, Vertex};
use std::sync::Arc;

pub struct MatchExecutor {
    storage: Arc<dyn GraphStorage>,
}

impl MatchExecutor {
    pub fn new(storage: Arc<dyn GraphStorage>) -> Self {
        Self { storage }
    }

    /// Execute a MATCH clause
    pub async fn execute(
        &self,
        match_clause: &MatchClause,
        where_clause: Option<&WhereClause>,
    ) -> ExecutionResult<Vec<Row>> {
        let mut results = Vec::new();

        // Execute each pattern
        for pattern in &match_clause.patterns {
            let pattern_results = self.execute_pattern(pattern).await?;
            results.extend(pattern_results);
        }

        // Apply WHERE filter
        if let Some(where_clause) = where_clause {
            results = self.apply_where_filter(results, &where_clause.condition)?;
        }

        Ok(results)
    }

    async fn execute_pattern(&self, pattern: &Pattern) -> ExecutionResult<Vec<Row>> {
        if pattern.elements.is_empty() {
            return Ok(vec![]);
        }

        // Analyze pattern structure
        let nodes: Vec<&NodePattern> = pattern
            .elements
            .iter()
            .filter_map(|e| match e {
                PatternElement::Node(n) => Some(n),
                _ => None,
            })
            .collect();

        let edges: Vec<&EdgePattern> = pattern
            .elements
            .iter()
            .filter_map(|e| match e {
                PatternElement::Edge(e) => Some(e),
                _ => None,
            })
            .collect();

        if edges.is_empty() {
            // Simple node-only pattern
            self.match_node_pattern(&nodes[0]).await
        } else {
            // Pattern with edges
            self.match_edge_pattern(pattern).await
        }
    }

    async fn match_node_pattern(&self, node: &NodePattern) -> ExecutionResult<Vec<Row>> {
        let label = node.label.as_deref().unwrap_or("");
        let vertices = self.storage.scan_vertices(label).await?;

        vertices
            .into_iter()
            .filter_map(|vertex| {
                match self.match_node_properties(&vertex, node) {
                    Ok(true) => {
                        let mut row = Row::new();
                        if let Some(var) = &node.variable {
                            row.insert(var.clone(), Value::Vertex(vertex));
                        }
                        Some(Ok(row))
                    }
                    Ok(false) => None,
                    Err(e) => Some(Err(e)),
                }
            })
            .collect()
    }

    async fn match_edge_pattern(&self, pattern: &Pattern) -> ExecutionResult<Vec<Row>> {
        let mut results = Vec::new();

        // Extract pattern structure: (start)-[edge]->(end)
        let elements = &pattern.elements;

        // Simple implementation: assume pattern is (node)-[edge]->(node)
        if elements.len() == 3 {
            if let (
                PatternElement::Node(start_node),
                PatternElement::Edge(edge_pattern),
                PatternElement::Node(end_node),
            ) = (&elements[0], &elements[1], &elements[2])
            {
                results = self
                    .match_triple_pattern(start_node, edge_pattern, end_node)
                    .await?;
            }
        } else if elements.len() == 5 {
            // (a)-[r1]->(b)-[r2]->(c)
            results = self.match_path_pattern(elements).await?;
        } else {
            return Err(ExecutionError::UnsupportedOperation(format!(
                "Pattern with {} elements not yet supported",
                elements.len()
            )));
        }

        Ok(results)
    }

    async fn match_triple_pattern(
        &self,
        start_node: &NodePattern,
        edge_pattern: &EdgePattern,
        end_node: &NodePattern,
    ) -> ExecutionResult<Vec<Row>> {
        let mut results = Vec::new();

        // Get start nodes
        let start_label = start_node.label.as_deref().unwrap_or("");
        let start_vertices = self.storage.scan_vertices(start_label).await?;

        for start_vertex in start_vertices {
            if !self.match_node_properties(&start_vertex, start_node)? {
                continue;
            }

            // Get outgoing edges based on direction
            let edges = match edge_pattern.direction {
                Direction::Right => self.storage.get_outgoing_edges(start_vertex.id).await?,
                Direction::Left => self.storage.get_incoming_edges(start_vertex.id).await?,
                Direction::Both => {
                    let mut all_edges = self.storage.get_outgoing_edges(start_vertex.id).await?;
                    all_edges.extend(self.storage.get_incoming_edges(start_vertex.id).await?);
                    all_edges
                }
            };

            // Filter edges by label and properties
            let edge_label = edge_pattern.label.as_deref().unwrap_or("");
            for edge in edges {
                if !edge_label.is_empty() && edge.label != edge_label {
                    continue;
                }

                if !self.match_edge_properties(&edge, edge_pattern)? {
                    continue;
                }

                // Get end vertex
                let end_vertex_id = match edge_pattern.direction {
                    Direction::Right => edge.end,
                    Direction::Left => edge.start,
                    Direction::Both => {
                        if edge.start == start_vertex.id {
                            edge.end
                        } else {
                            edge.start
                        }
                    }
                };

                if let Some(end_vertex) = self.storage.get_vertex(end_vertex_id).await? {
                    // Check end node constraints
                    if let Some(end_label) = &end_node.label {
                        if &end_vertex.label != end_label {
                            continue;
                        }
                    }

                    if !self.match_node_properties(&end_vertex, end_node)? {
                        continue;
                    }

                    // Build result row
                    let mut row = Row::new();

                    if let Some(var) = &start_node.variable {
                        row.insert(var.clone(), Value::Vertex(start_vertex.clone()));
                    }

                    if let Some(var) = &edge_pattern.variable {
                        row.insert(var.clone(), Value::Edge(edge.clone()));
                    }

                    if let Some(var) = &end_node.variable {
                        row.insert(var.clone(), Value::Vertex(end_vertex));
                    }

                    results.push(row);
                }
            }
        }

        Ok(results)
    }

    async fn match_path_pattern(&self, elements: &[PatternElement]) -> ExecutionResult<Vec<Row>> {
        // For now, simple implementation for 2-hop paths
        // TODO: Generalize to arbitrary length paths

        let mut results = Vec::new();

        if elements.len() != 5 {
            return Err(ExecutionError::UnsupportedOperation(
                "Only 2-hop paths supported".to_string(),
            ));
        }

        // Extract nodes and edges
        let nodes: Vec<&NodePattern> = elements
            .iter()
            .filter_map(|e| match e {
                PatternElement::Node(n) => Some(n),
                _ => None,
            })
            .collect();

        let edges: Vec<&EdgePattern> = elements
            .iter()
            .filter_map(|e| match e {
                PatternElement::Edge(e) => Some(e),
                _ => None,
            })
            .collect();

        if nodes.len() != 3 || edges.len() != 2 {
            return Err(ExecutionError::InvalidExpression(
                "Invalid path pattern".to_string(),
            ));
        }

        // Match first hop
        let first_hop_results = self.match_triple_pattern(nodes[0], edges[0], nodes[1]).await?;

        // For each first hop result, match second hop
        for first_row in first_hop_results {
            // Get the middle node
            let middle_var = nodes[1].variable.as_ref()
                .ok_or_else(|| ExecutionError::InvalidExpression(
                    "Middle node in path pattern must have a variable".to_string()
                ))?;
            let middle_vertex = first_row.get(middle_var)
                .ok_or_else(|| ExecutionError::VariableNotFound(middle_var.clone()))?
                .as_vertex()?;

            // Match second hop starting from middle node
            let second_edges = match edges[1].direction {
                Direction::Right => self.storage.get_outgoing_edges(middle_vertex.id).await?,
                Direction::Left => self.storage.get_incoming_edges(middle_vertex.id).await?,
                Direction::Both => {
                    let mut all = self.storage.get_outgoing_edges(middle_vertex.id).await?;
                    all.extend(self.storage.get_incoming_edges(middle_vertex.id).await?);
                    all
                }
            };

            for edge in second_edges {
                if !self.match_edge_properties(&edge, edges[1])? {
                    continue;
                }

                let end_vertex_id = match edges[1].direction {
                    Direction::Right => edge.end,
                    Direction::Left => edge.start,
                    Direction::Both => {
                        if edge.start == middle_vertex.id {
                            edge.end
                        } else {
                            edge.start
                        }
                    }
                };

                if let Some(end_vertex) = self.storage.get_vertex(end_vertex_id).await? {
                    if !self.match_node_properties(&end_vertex, nodes[2])? {
                        continue;
                    }

                    // Build result row combining both hops
                    let mut row = first_row.clone();

                    if let Some(var) = &edges[1].variable {
                        row.insert(var.clone(), Value::Edge(edge));
                    }

                    if let Some(var) = &nodes[2].variable {
                        row.insert(var.clone(), Value::Vertex(end_vertex));
                    }

                    results.push(row);
                }
            }
        }

        Ok(results)
    }

    fn match_node_properties(
        &self,
        vertex: &Vertex,
        pattern: &NodePattern,
    ) -> ExecutionResult<bool> {
        if let Some(props) = &pattern.properties {
            for (key, expr) in props {
                let pattern_value = self.evaluate_literal(expr)?;
                let vertex_value = vertex
                    .properties
                    .get(key)
                    .ok_or_else(|| ExecutionError::PropertyNotFound(key.clone()))?;

                if !self.values_equal(&pattern_value, vertex_value) {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    fn match_edge_properties(
        &self,
        edge: &Edge,
        pattern: &EdgePattern,
    ) -> ExecutionResult<bool> {
        if let Some(props) = &pattern.properties {
            for (key, expr) in props {
                let pattern_value = self.evaluate_literal(expr)?;
                let edge_value = edge
                    .properties
                    .get(key)
                    .ok_or_else(|| ExecutionError::PropertyNotFound(key.clone()))?;

                if !self.values_equal(&pattern_value, edge_value) {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    fn evaluate_literal(&self, expr: &Expression) -> ExecutionResult<serde_json::Value> {
        match expr {
            Expression::Literal(lit) => self.literal_to_json(lit),
            _ => Err(ExecutionError::InvalidExpression(
                "Only literals supported in property patterns".to_string(),
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
                    .map(|e| self.evaluate_literal(e))
                    .collect();
                Ok(serde_json::Value::Array(values?))
            }
            Literal::Map(map) => {
                let obj: ExecutionResult<serde_json::Map<String, serde_json::Value>> = map
                    .iter()
                    .map(|(k, v)| Ok((k.clone(), self.evaluate_literal(v)?)))
                    .collect();
                Ok(serde_json::Value::Object(obj?))
            }
        }
    }

    fn values_equal(&self, val1: &serde_json::Value, val2: &serde_json::Value) -> bool {
        val1 == val2
    }

    fn apply_where_filter(
        &self,
        rows: Vec<Row>,
        condition: &Expression,
    ) -> ExecutionResult<Vec<Row>> {
        let mut filtered = Vec::new();

        for row in rows {
            let result = self.evaluate_expression(condition, &row)?;
            if self.is_truthy(&result) {
                filtered.push(row);
            }
        }

        Ok(filtered)
    }

    /// Evaluate an expression in the context of a row
    fn evaluate_expression(&self, expr: &Expression, row: &Row) -> ExecutionResult<Value> {
        match expr {
            Expression::Literal(lit) => Ok(self.literal_to_value(lit)),

            Expression::Variable(name) => {
                row.get(name)
                    .cloned()
                    .ok_or_else(|| ExecutionError::VariableNotFound(name.clone()))
            }

            Expression::Property(prop_expr) => {
                self.evaluate_property(prop_expr, row)
            }

            Expression::BinaryOp { left, op, right } => {
                let left_val = self.evaluate_expression(left, row)?;
                let right_val = self.evaluate_expression(right, row)?;
                self.evaluate_binary_op(&left_val, op, &right_val)
            }

            Expression::UnaryOp { op, expr } => {
                let val = self.evaluate_expression(expr, row)?;
                self.evaluate_unary_op(op, &val)
            }

            Expression::Parameter(_) => {
                Err(ExecutionError::UnsupportedOperation(
                    "Parameters not yet supported".to_string(),
                ))
            }

            Expression::Index { .. } => {
                Err(ExecutionError::UnsupportedOperation(
                    "Index expressions not yet supported".to_string(),
                ))
            }

            Expression::FunctionCall { name, .. } => {
                Err(ExecutionError::UnsupportedOperation(
                    format!("Function '{}' not yet supported", name),
                ))
            }
        }
    }

    /// Evaluate a property expression (e.g., p.age)
    fn evaluate_property(&self, prop_expr: &PropertyExpression, row: &Row) -> ExecutionResult<Value> {
        let base_value = row.get(&prop_expr.base)
            .ok_or_else(|| ExecutionError::VariableNotFound(prop_expr.base.clone()))?;

        // Get the JSON properties from the vertex/edge
        let json = match base_value {
            Value::Vertex(v) => &v.properties,
            Value::Edge(e) => &e.properties,
            _ => {
                return Err(ExecutionError::TypeMismatch {
                    expected: "Vertex or Edge".to_string(),
                    actual: format!("{:?}", base_value),
                });
            }
        };

        // Navigate through property path
        let mut current = json;
        for prop in &prop_expr.properties {
            current = current.get(prop)
                .ok_or_else(|| ExecutionError::PropertyNotFound(prop.clone()))?;
        }

        Ok(self.json_to_value(current))
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

    /// Convert JSON to Value
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
            serde_json::Value::Object(obj) => {
                Value::Map(
                    obj.iter()
                        .map(|(k, v)| (k.clone(), self.json_to_value(v)))
                        .collect()
                )
            }
        }
    }

    /// Evaluate a binary operation
    fn evaluate_binary_op(
        &self,
        left: &Value,
        op: &BinaryOperator,
        right: &Value,
    ) -> ExecutionResult<Value> {
        match op {
            // Comparison operators
            BinaryOperator::Eq => Ok(Value::Boolean(self.values_eq(left, right))),
            BinaryOperator::Neq => Ok(Value::Boolean(!self.values_eq(left, right))),
            BinaryOperator::Lt => self.compare_values(left, right, |ord| ord == std::cmp::Ordering::Less),
            BinaryOperator::Gt => self.compare_values(left, right, |ord| ord == std::cmp::Ordering::Greater),
            BinaryOperator::Lte => self.compare_values(left, right, |ord| ord != std::cmp::Ordering::Greater),
            BinaryOperator::Gte => self.compare_values(left, right, |ord| ord != std::cmp::Ordering::Less),

            // Logical operators
            BinaryOperator::And => {
                Ok(Value::Boolean(self.is_truthy(left) && self.is_truthy(right)))
            }
            BinaryOperator::Or => {
                Ok(Value::Boolean(self.is_truthy(left) || self.is_truthy(right)))
            }

            // Arithmetic operators
            BinaryOperator::Add => self.arithmetic_op(left, right, |a, b| a + b, |a, b| a + b),
            BinaryOperator::Subtract => self.arithmetic_op(left, right, |a, b| a - b, |a, b| a - b),
            BinaryOperator::Multiply => self.arithmetic_op(left, right, |a, b| a * b, |a, b| a * b),
            BinaryOperator::Divide => {
                // Check for division by zero
                match right {
                    Value::Integer(0) => {
                        Err(ExecutionError::InvalidExpression("Division by zero".to_string()))
                    }
                    Value::Float(f) if *f == 0.0 => {
                        Err(ExecutionError::InvalidExpression("Division by zero".to_string()))
                    }
                    _ => self.arithmetic_op(left, right, |a, b| a / b, |a, b| a / b),
                }
            }
            BinaryOperator::Modulo => {
                match (left, right) {
                    (Value::Integer(a), Value::Integer(b)) if *b != 0 => {
                        Ok(Value::Integer(a % b))
                    }
                    _ => Err(ExecutionError::InvalidExpression(
                        "Modulo requires integer operands".to_string(),
                    )),
                }
            }
        }
    }

    /// Evaluate a unary operation
    fn evaluate_unary_op(&self, op: &UnaryOperator, val: &Value) -> ExecutionResult<Value> {
        match op {
            UnaryOperator::Not => Ok(Value::Boolean(!self.is_truthy(val))),
            UnaryOperator::Minus => {
                match val {
                    Value::Integer(i) => Ok(Value::Integer(-i)),
                    Value::Float(f) => Ok(Value::Float(-f)),
                    _ => Err(ExecutionError::TypeMismatch {
                        expected: "Number".to_string(),
                        actual: format!("{:?}", val),
                    }),
                }
            }
            UnaryOperator::Plus => {
                match val {
                    Value::Integer(_) | Value::Float(_) => Ok(val.clone()),
                    _ => Err(ExecutionError::TypeMismatch {
                        expected: "Number".to_string(),
                        actual: format!("{:?}", val),
                    }),
                }
            }
        }
    }

    /// Check if two values are equal
    fn values_eq(&self, left: &Value, right: &Value) -> bool {
        match (left, right) {
            (Value::Null, Value::Null) => true,
            (Value::Boolean(a), Value::Boolean(b)) => a == b,
            (Value::Integer(a), Value::Integer(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => (a - b).abs() < f64::EPSILON,
            (Value::Integer(a), Value::Float(b)) | (Value::Float(b), Value::Integer(a)) => {
                (*a as f64 - b).abs() < f64::EPSILON
            }
            (Value::String(a), Value::String(b)) => a == b,
            _ => false,
        }
    }

    /// Compare two values and apply a predicate to the ordering
    fn compare_values<F>(&self, left: &Value, right: &Value, pred: F) -> ExecutionResult<Value>
    where
        F: Fn(std::cmp::Ordering) -> bool,
    {
        let ordering = match (left, right) {
            (Value::Integer(a), Value::Integer(b)) => a.cmp(b),
            (Value::Float(a), Value::Float(b)) => {
                a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
            }
            (Value::Integer(a), Value::Float(b)) => {
                (*a as f64).partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
            }
            (Value::Float(a), Value::Integer(b)) => {
                a.partial_cmp(&(*b as f64)).unwrap_or(std::cmp::Ordering::Equal)
            }
            (Value::String(a), Value::String(b)) => a.cmp(b),
            _ => {
                return Err(ExecutionError::TypeMismatch {
                    expected: "Comparable types".to_string(),
                    actual: format!("{:?} vs {:?}", left, right),
                });
            }
        };

        Ok(Value::Boolean(pred(ordering)))
    }

    /// Perform an arithmetic operation
    fn arithmetic_op<F, G>(
        &self,
        left: &Value,
        right: &Value,
        int_op: F,
        float_op: G,
    ) -> ExecutionResult<Value>
    where
        F: Fn(i64, i64) -> i64,
        G: Fn(f64, f64) -> f64,
    {
        match (left, right) {
            (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(int_op(*a, *b))),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(float_op(*a, *b))),
            (Value::Integer(a), Value::Float(b)) => Ok(Value::Float(float_op(*a as f64, *b))),
            (Value::Float(a), Value::Integer(b)) => Ok(Value::Float(float_op(*a, *b as f64))),
            (Value::String(a), Value::String(b)) if matches!(int_op(1, 0), 1) => {
                // String concatenation for Add
                Ok(Value::String(format!("{}{}", a, b)))
            }
            _ => Err(ExecutionError::TypeMismatch {
                expected: "Number".to_string(),
                actual: format!("{:?} and {:?}", left, right),
            }),
        }
    }

    /// Check if a value is truthy
    fn is_truthy(&self, val: &Value) -> bool {
        match val {
            Value::Null => false,
            Value::Boolean(b) => *b,
            Value::Integer(i) => *i != 0,
            Value::Float(f) => *f != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::List(l) => !l.is_empty(),
            Value::Map(m) => !m.is_empty(),
            Value::Vertex(_) => true,
            Value::Edge(_) => true,
            Value::Path(p) => !p.is_empty(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{rocksdb_store::RocksDbStorage, GraphStorage};
    use std::collections::HashMap;
    use tempfile::TempDir;

    async fn setup_test_storage() -> (Arc<dyn GraphStorage>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage: Arc<dyn GraphStorage> = Arc::new(RocksDbStorage::new(temp_dir.path(), "test_graph").unwrap());
        (storage, temp_dir)
    }

    #[tokio::test]
    async fn test_match_simple_node() {
        let (storage, _temp) = setup_test_storage().await;

        // Create test data
        let mut tx = storage.begin_transaction().await.unwrap();
        tx.create_vertex("Person", serde_json::json!({"name": "Alice"}))
            .await
            .unwrap();
        tx.create_vertex("Person", serde_json::json!({"name": "Bob"}))
            .await
            .unwrap();
        tx.commit().await.unwrap();

        // Execute MATCH
        let executor = MatchExecutor::new(storage.clone());
        let pattern = Pattern {
            elements: vec![PatternElement::Node(NodePattern {
                variable: Some("n".to_string()),
                label: Some("Person".to_string()),
                properties: None,
            })],
        };

        let match_clause = MatchClause {
            patterns: vec![pattern],
            optional: false,
        };

        let results = executor.execute(&match_clause, None).await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_match_with_properties() {
        let (storage, _temp) = setup_test_storage().await;

        // Create test data
        let mut tx = storage.begin_transaction().await.unwrap();
        tx.create_vertex("Person", serde_json::json!({"name": "Alice", "age": 30}))
            .await
            .unwrap();
        tx.create_vertex("Person", serde_json::json!({"name": "Bob", "age": 25}))
            .await
            .unwrap();
        tx.commit().await.unwrap();

        // Execute MATCH with property constraint
        let executor = MatchExecutor::new(storage.clone());
        let mut props = HashMap::new();
        props.insert(
            "name".to_string(),
            Expression::Literal(Literal::String("Alice".to_string())),
        );

        let pattern = Pattern {
            elements: vec![PatternElement::Node(NodePattern {
                variable: Some("n".to_string()),
                label: Some("Person".to_string()),
                properties: Some(props),
            })],
        };

        let match_clause = MatchClause {
            patterns: vec![pattern],
            optional: false,
        };

        let results = executor.execute(&match_clause, None).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_where_greater_than() {
        let (storage, _temp) = setup_test_storage().await;

        // Create test data
        let mut tx = storage.begin_transaction().await.unwrap();
        tx.create_vertex("Person", serde_json::json!({"name": "Alice", "age": 30}))
            .await
            .unwrap();
        tx.create_vertex("Person", serde_json::json!({"name": "Bob", "age": 25}))
            .await
            .unwrap();
        tx.create_vertex("Person", serde_json::json!({"name": "Charlie", "age": 35}))
            .await
            .unwrap();
        tx.commit().await.unwrap();

        // Execute MATCH with WHERE p.age > 28
        let executor = MatchExecutor::new(storage.clone());
        let pattern = Pattern {
            elements: vec![PatternElement::Node(NodePattern {
                variable: Some("p".to_string()),
                label: Some("Person".to_string()),
                properties: None,
            })],
        };

        let match_clause = MatchClause {
            patterns: vec![pattern],
            optional: false,
        };

        // WHERE p.age > 28
        let where_clause = WhereClause {
            condition: Expression::BinaryOp {
                left: Box::new(Expression::Property(PropertyExpression {
                    base: "p".to_string(),
                    properties: vec!["age".to_string()],
                })),
                op: BinaryOperator::Gt,
                right: Box::new(Expression::Literal(Literal::Integer(28))),
            },
        };

        let results = executor.execute(&match_clause, Some(&where_clause)).await.unwrap();
        assert_eq!(results.len(), 2); // Alice (30) and Charlie (35)
    }

    #[tokio::test]
    async fn test_where_equals() {
        let (storage, _temp) = setup_test_storage().await;

        // Create test data
        let mut tx = storage.begin_transaction().await.unwrap();
        tx.create_vertex("Person", serde_json::json!({"name": "Alice", "age": 30}))
            .await
            .unwrap();
        tx.create_vertex("Person", serde_json::json!({"name": "Bob", "age": 25}))
            .await
            .unwrap();
        tx.commit().await.unwrap();

        // Execute MATCH with WHERE p.name = 'Alice'
        let executor = MatchExecutor::new(storage.clone());
        let pattern = Pattern {
            elements: vec![PatternElement::Node(NodePattern {
                variable: Some("p".to_string()),
                label: Some("Person".to_string()),
                properties: None,
            })],
        };

        let match_clause = MatchClause {
            patterns: vec![pattern],
            optional: false,
        };

        // WHERE p.name = 'Alice'
        let where_clause = WhereClause {
            condition: Expression::BinaryOp {
                left: Box::new(Expression::Property(PropertyExpression {
                    base: "p".to_string(),
                    properties: vec!["name".to_string()],
                })),
                op: BinaryOperator::Eq,
                right: Box::new(Expression::Literal(Literal::String("Alice".to_string()))),
            },
        };

        let results = executor.execute(&match_clause, Some(&where_clause)).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_where_and() {
        let (storage, _temp) = setup_test_storage().await;

        // Create test data
        let mut tx = storage.begin_transaction().await.unwrap();
        tx.create_vertex("Person", serde_json::json!({"name": "Alice", "age": 30, "city": "Beijing"}))
            .await
            .unwrap();
        tx.create_vertex("Person", serde_json::json!({"name": "Bob", "age": 30, "city": "Shanghai"}))
            .await
            .unwrap();
        tx.create_vertex("Person", serde_json::json!({"name": "Charlie", "age": 25, "city": "Beijing"}))
            .await
            .unwrap();
        tx.commit().await.unwrap();

        // Execute MATCH with WHERE p.age = 30 AND p.city = 'Beijing'
        let executor = MatchExecutor::new(storage.clone());
        let pattern = Pattern {
            elements: vec![PatternElement::Node(NodePattern {
                variable: Some("p".to_string()),
                label: Some("Person".to_string()),
                properties: None,
            })],
        };

        let match_clause = MatchClause {
            patterns: vec![pattern],
            optional: false,
        };

        // WHERE p.age = 30 AND p.city = 'Beijing'
        let where_clause = WhereClause {
            condition: Expression::BinaryOp {
                left: Box::new(Expression::BinaryOp {
                    left: Box::new(Expression::Property(PropertyExpression {
                        base: "p".to_string(),
                        properties: vec!["age".to_string()],
                    })),
                    op: BinaryOperator::Eq,
                    right: Box::new(Expression::Literal(Literal::Integer(30))),
                }),
                op: BinaryOperator::And,
                right: Box::new(Expression::BinaryOp {
                    left: Box::new(Expression::Property(PropertyExpression {
                        base: "p".to_string(),
                        properties: vec!["city".to_string()],
                    })),
                    op: BinaryOperator::Eq,
                    right: Box::new(Expression::Literal(Literal::String("Beijing".to_string()))),
                }),
            },
        };

        let results = executor.execute(&match_clause, Some(&where_clause)).await.unwrap();
        assert_eq!(results.len(), 1); // Only Alice
    }

    #[tokio::test]
    async fn test_where_or() {
        let (storage, _temp) = setup_test_storage().await;

        // Create test data
        let mut tx = storage.begin_transaction().await.unwrap();
        tx.create_vertex("Person", serde_json::json!({"name": "Alice", "age": 30}))
            .await
            .unwrap();
        tx.create_vertex("Person", serde_json::json!({"name": "Bob", "age": 25}))
            .await
            .unwrap();
        tx.create_vertex("Person", serde_json::json!({"name": "Charlie", "age": 35}))
            .await
            .unwrap();
        tx.commit().await.unwrap();

        // Execute MATCH with WHERE p.age < 26 OR p.age > 34
        let executor = MatchExecutor::new(storage.clone());
        let pattern = Pattern {
            elements: vec![PatternElement::Node(NodePattern {
                variable: Some("p".to_string()),
                label: Some("Person".to_string()),
                properties: None,
            })],
        };

        let match_clause = MatchClause {
            patterns: vec![pattern],
            optional: false,
        };

        // WHERE p.age < 26 OR p.age > 34
        let where_clause = WhereClause {
            condition: Expression::BinaryOp {
                left: Box::new(Expression::BinaryOp {
                    left: Box::new(Expression::Property(PropertyExpression {
                        base: "p".to_string(),
                        properties: vec!["age".to_string()],
                    })),
                    op: BinaryOperator::Lt,
                    right: Box::new(Expression::Literal(Literal::Integer(26))),
                }),
                op: BinaryOperator::Or,
                right: Box::new(Expression::BinaryOp {
                    left: Box::new(Expression::Property(PropertyExpression {
                        base: "p".to_string(),
                        properties: vec!["age".to_string()],
                    })),
                    op: BinaryOperator::Gt,
                    right: Box::new(Expression::Literal(Literal::Integer(34))),
                }),
            },
        };

        let results = executor.execute(&match_clause, Some(&where_clause)).await.unwrap();
        assert_eq!(results.len(), 2); // Bob (25) and Charlie (35)
    }

    #[tokio::test]
    async fn test_where_not() {
        let (storage, _temp) = setup_test_storage().await;

        // Create test data
        let mut tx = storage.begin_transaction().await.unwrap();
        tx.create_vertex("Person", serde_json::json!({"name": "Alice", "active": true}))
            .await
            .unwrap();
        tx.create_vertex("Person", serde_json::json!({"name": "Bob", "active": false}))
            .await
            .unwrap();
        tx.commit().await.unwrap();

        // Execute MATCH with WHERE NOT p.active
        let executor = MatchExecutor::new(storage.clone());
        let pattern = Pattern {
            elements: vec![PatternElement::Node(NodePattern {
                variable: Some("p".to_string()),
                label: Some("Person".to_string()),
                properties: None,
            })],
        };

        let match_clause = MatchClause {
            patterns: vec![pattern],
            optional: false,
        };

        // WHERE NOT p.active
        let where_clause = WhereClause {
            condition: Expression::UnaryOp {
                op: UnaryOperator::Not,
                expr: Box::new(Expression::Property(PropertyExpression {
                    base: "p".to_string(),
                    properties: vec!["active".to_string()],
                })),
            },
        };

        let results = executor.execute(&match_clause, Some(&where_clause)).await.unwrap();
        assert_eq!(results.len(), 1); // Only Bob
    }
}
