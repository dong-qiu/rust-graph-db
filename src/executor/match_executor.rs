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

        let mut results = Vec::new();

        for vertex in vertices {
            // Check property constraints
            if self.match_node_properties(&vertex, node)? {
                let mut row = Row::new();
                if let Some(var) = &node.variable {
                    row.insert(var.clone(), Value::Vertex(vertex));
                }
                results.push(row);
            }
        }

        Ok(results)
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
            let middle_var = nodes[1].variable.as_ref().unwrap();
            let middle_vertex = first_row.get(middle_var).unwrap().as_vertex()?;

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
            Expression::Literal(lit) => Ok(self.literal_to_json(lit)),
            _ => Err(ExecutionError::InvalidExpression(
                "Only literals supported in property patterns".to_string(),
            )),
        }
    }

    fn literal_to_json(&self, lit: &Literal) -> serde_json::Value {
        match lit {
            Literal::Null => serde_json::Value::Null,
            Literal::Boolean(b) => serde_json::Value::Bool(*b),
            Literal::Integer(i) => serde_json::Value::Number((*i).into()),
            Literal::Float(f) => {
                serde_json::Value::Number(serde_json::Number::from_f64(*f).unwrap())
            }
            Literal::String(s) => serde_json::Value::String(s.clone()),
            Literal::List(items) => {
                serde_json::Value::Array(items.iter().map(|e| self.evaluate_literal(e).unwrap()).collect())
            }
            Literal::Map(map) => {
                let obj: serde_json::Map<String, serde_json::Value> = map
                    .iter()
                    .map(|(k, v)| (k.clone(), self.evaluate_literal(v).unwrap()))
                    .collect();
                serde_json::Value::Object(obj)
            }
        }
    }

    fn values_equal(&self, val1: &serde_json::Value, val2: &serde_json::Value) -> bool {
        val1 == val2
    }

    fn apply_where_filter(
        &self,
        rows: Vec<Row>,
        _condition: &Expression,
    ) -> ExecutionResult<Vec<Row>> {
        // TODO: Implement WHERE clause evaluation
        // For now, just return all rows
        Ok(rows)
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
        };

        let results = executor.execute(&match_clause, None).await.unwrap();
        assert_eq!(results.len(), 1);
    }
}
