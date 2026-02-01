use super::graphid::Graphid;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// Vertex (Node) in the graph
///
/// Represents a graph vertex with:
/// - Unique identifier (Graphid)
/// - Label (type/class of the vertex)
/// - Properties (arbitrary JSON data)
///
/// This matches the openGauss-graph Vertex structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Vertex {
    /// Unique identifier
    pub id: Graphid,

    /// Vertex label (e.g., "Person", "Company")
    pub label: String,

    /// Properties stored as JSON
    pub properties: JsonValue,
}

impl Vertex {
    /// Create a new vertex
    pub fn new(id: Graphid, label: impl Into<String>, properties: JsonValue) -> Self {
        Self {
            id,
            label: label.into(),
            properties,
        }
    }

    /// Create a vertex with an empty property map
    pub fn new_empty(id: Graphid, label: impl Into<String>) -> Self {
        Self::new(id, label, JsonValue::Object(serde_json::Map::new()))
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

    /// Check if vertex has a specific property
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_vertex_creation() {
        let id = Graphid::new(1, 100).unwrap();
        let props = json!({
            "name": "Alice",
            "age": 30
        });

        let vertex = Vertex::new(id, "Person", props);

        assert_eq!(vertex.id, id);
        assert_eq!(vertex.label, "Person");
        assert_eq!(vertex.get_property("name"), Some(&json!("Alice")));
        assert_eq!(vertex.get_property("age"), Some(&json!(30)));
    }

    #[test]
    fn test_vertex_empty() {
        let id = Graphid::new(1, 100).unwrap();
        let vertex = Vertex::new_empty(id, "Person");

        assert_eq!(vertex.id, id);
        assert_eq!(vertex.label, "Person");
        assert!(vertex.property_keys().is_empty());
    }

    #[test]
    fn test_vertex_set_property() {
        let id = Graphid::new(1, 100).unwrap();
        let mut vertex = Vertex::new_empty(id, "Person");

        vertex.set_property("name", json!("Bob"));
        vertex.set_property("age", json!(25));

        assert_eq!(vertex.get_property("name"), Some(&json!("Bob")));
        assert_eq!(vertex.get_property("age"), Some(&json!(25)));
    }

    #[test]
    fn test_vertex_remove_property() {
        let id = Graphid::new(1, 100).unwrap();
        let props = json!({
            "name": "Alice",
            "age": 30
        });
        let mut vertex = Vertex::new(id, "Person", props);

        let removed = vertex.remove_property("age");
        assert_eq!(removed, Some(json!(30)));
        assert!(!vertex.has_property("age"));
        assert!(vertex.has_property("name"));
    }

    #[test]
    fn test_vertex_property_keys() {
        let id = Graphid::new(1, 100).unwrap();
        let props = json!({
            "name": "Alice",
            "age": 30,
            "city": "Beijing"
        });
        let vertex = Vertex::new(id, "Person", props);

        let mut keys = vertex.property_keys();
        keys.sort();
        assert_eq!(keys, vec!["age", "city", "name"]);
    }

    #[test]
    fn test_vertex_serialization() {
        let id = Graphid::new(1, 100).unwrap();
        let props = json!({
            "name": "Alice",
            "age": 30
        });
        let vertex = Vertex::new(id, "Person", props);

        // Serialize to JSON
        let serialized = serde_json::to_string(&vertex).unwrap();

        // Deserialize back
        let deserialized: Vertex = serde_json::from_str(&serialized).unwrap();

        assert_eq!(vertex, deserialized);
    }
}
