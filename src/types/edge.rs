use super::graphid::Graphid;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// Edge (Relationship) in the graph
///
/// Represents a directed edge with:
/// - Unique identifier (Graphid)
/// - Start vertex ID
/// - End vertex ID
/// - Label (type/class of the edge)
/// - Properties (arbitrary JSON data)
///
/// This matches the openGauss-graph Edge structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Edge {
    /// Unique identifier
    pub id: Graphid,

    /// Start vertex ID (source)
    pub start: Graphid,

    /// End vertex ID (target)
    pub end: Graphid,

    /// Edge label (e.g., "KNOWS", "WORKS_FOR")
    pub label: String,

    /// Properties stored as JSON
    pub properties: JsonValue,
}

impl Edge {
    /// Create a new edge
    pub fn new(
        id: Graphid,
        start: Graphid,
        end: Graphid,
        label: impl Into<String>,
        properties: JsonValue,
    ) -> Self {
        Self {
            id,
            start,
            end,
            label: label.into(),
            properties,
        }
    }

    /// Create an edge with an empty property map
    pub fn new_empty(
        id: Graphid,
        start: Graphid,
        end: Graphid,
        label: impl Into<String>,
    ) -> Self {
        Self::new(
            id,
            start,
            end,
            label,
            JsonValue::Object(serde_json::Map::new()),
        )
    }

    /// Get a property value by key
    pub fn get_property(&self, key: &str) -> Option<&JsonValue> {
        self.properties.get(key)
    }

    /// Set a property value
    pub fn set_property(&mut self, key: impl Into<String>, value: JsonValue) {
        if let JsonValue::Object(ref mut map) = self.properties {
            map.insert(key.into(), value);
        }
    }

    /// Remove a property
    pub fn remove_property(&mut self, key: &str) -> Option<JsonValue> {
        if let JsonValue::Object(ref mut map) = self.properties {
            map.remove(key)
        } else {
            None
        }
    }

    /// Get all property keys
    pub fn property_keys(&self) -> Vec<String> {
        if let JsonValue::Object(map) = &self.properties {
            map.keys().cloned().collect()
        } else {
            Vec::new()
        }
    }

    /// Check if edge has a specific property
    pub fn has_property(&self, key: &str) -> bool {
        self.properties.get(key).is_some()
    }

    /// Convert properties to a HashMap
    pub fn properties_as_map(&self) -> HashMap<String, JsonValue> {
        if let JsonValue::Object(map) = &self.properties {
            map.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        } else {
            HashMap::new()
        }
    }

    /// Check if this edge is a self-loop
    pub fn is_self_loop(&self) -> bool {
        self.start == self.end
    }

    /// Reverse the edge direction
    pub fn reverse(&self) -> Self {
        Self {
            id: self.id,
            start: self.end,
            end: self.start,
            label: self.label.clone(),
            properties: self.properties.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_edge_creation() {
        let id = Graphid::new(2, 200).unwrap();
        let start = Graphid::new(1, 100).unwrap();
        let end = Graphid::new(1, 101).unwrap();
        let props = json!({
            "since": 2020,
            "weight": 1.5
        });

        let edge = Edge::new(id, start, end, "KNOWS", props);

        assert_eq!(edge.id, id);
        assert_eq!(edge.start, start);
        assert_eq!(edge.end, end);
        assert_eq!(edge.label, "KNOWS");
        assert_eq!(edge.get_property("since"), Some(&json!(2020)));
        assert_eq!(edge.get_property("weight"), Some(&json!(1.5)));
    }

    #[test]
    fn test_edge_empty() {
        let id = Graphid::new(2, 200).unwrap();
        let start = Graphid::new(1, 100).unwrap();
        let end = Graphid::new(1, 101).unwrap();

        let edge = Edge::new_empty(id, start, end, "KNOWS");

        assert_eq!(edge.id, id);
        assert_eq!(edge.start, start);
        assert_eq!(edge.end, end);
        assert_eq!(edge.label, "KNOWS");
        assert!(edge.property_keys().is_empty());
    }

    #[test]
    fn test_edge_set_property() {
        let id = Graphid::new(2, 200).unwrap();
        let start = Graphid::new(1, 100).unwrap();
        let end = Graphid::new(1, 101).unwrap();
        let mut edge = Edge::new_empty(id, start, end, "KNOWS");

        edge.set_property("since", json!(2020));
        edge.set_property("weight", json!(1.5));

        assert_eq!(edge.get_property("since"), Some(&json!(2020)));
        assert_eq!(edge.get_property("weight"), Some(&json!(1.5)));
    }

    #[test]
    fn test_edge_remove_property() {
        let id = Graphid::new(2, 200).unwrap();
        let start = Graphid::new(1, 100).unwrap();
        let end = Graphid::new(1, 101).unwrap();
        let props = json!({
            "since": 2020,
            "weight": 1.5
        });
        let mut edge = Edge::new(id, start, end, "KNOWS", props);

        let removed = edge.remove_property("weight");
        assert_eq!(removed, Some(json!(1.5)));
        assert!(!edge.has_property("weight"));
        assert!(edge.has_property("since"));
    }

    #[test]
    fn test_edge_is_self_loop() {
        let id = Graphid::new(2, 200).unwrap();
        let node = Graphid::new(1, 100).unwrap();

        let self_loop = Edge::new_empty(id, node, node, "LIKES");
        assert!(self_loop.is_self_loop());

        let other_node = Graphid::new(1, 101).unwrap();
        let regular_edge = Edge::new_empty(id, node, other_node, "KNOWS");
        assert!(!regular_edge.is_self_loop());
    }

    #[test]
    fn test_edge_reverse() {
        let id = Graphid::new(2, 200).unwrap();
        let start = Graphid::new(1, 100).unwrap();
        let end = Graphid::new(1, 101).unwrap();
        let props = json!({"since": 2020});

        let edge = Edge::new(id, start, end, "KNOWS", props);
        let reversed = edge.reverse();

        assert_eq!(reversed.id, edge.id);
        assert_eq!(reversed.start, edge.end);
        assert_eq!(reversed.end, edge.start);
        assert_eq!(reversed.label, edge.label);
        assert_eq!(reversed.properties, edge.properties);
    }

    #[test]
    fn test_edge_serialization() {
        let id = Graphid::new(2, 200).unwrap();
        let start = Graphid::new(1, 100).unwrap();
        let end = Graphid::new(1, 101).unwrap();
        let props = json!({
            "since": 2020,
            "weight": 1.5
        });
        let edge = Edge::new(id, start, end, "KNOWS", props);

        // Serialize to JSON
        let serialized = serde_json::to_string(&edge).unwrap();

        // Deserialize back
        let deserialized: Edge = serde_json::from_str(&serialized).unwrap();

        assert_eq!(edge, deserialized);
    }
}
