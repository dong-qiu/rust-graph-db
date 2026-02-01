use super::{edge::Edge, graphid::Graphid, vertex::Vertex};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error types for GraphPath operations
#[derive(Error, Debug)]
pub enum PathError {
    #[error("Path is empty")]
    EmptyPath,

    #[error("Path vertices and edges count mismatch: {vertices} vertices, {edges} edges")]
    CountMismatch { vertices: usize, edges: usize },

    #[error("Path discontinuity at position {pos}: edge connects {edge_start} to {edge_end}, but path has {path_end}")]
    Discontinuity {
        pos: usize,
        edge_start: Graphid,
        edge_end: Graphid,
        path_end: Graphid,
    },
}

/// GraphPath represents a path in the graph
///
/// A path consists of an alternating sequence of vertices and edges:
/// (v1) -[e1]-> (v2) -[e2]-> (v3) ... -[en]-> (vn+1)
///
/// Invariants:
/// - vertices.len() = edges.len() + 1
/// - edges[i] connects vertices[i] to vertices[i+1]
///
/// This matches the openGauss-graph GraphPath structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphPath {
    /// Vertices in the path
    pub vertices: Vec<Vertex>,

    /// Edges in the path
    pub edges: Vec<Edge>,
}

impl GraphPath {
    /// Create a new path with a single vertex (no edges)
    pub fn new(start: Vertex) -> Self {
        Self {
            vertices: vec![start],
            edges: Vec::new(),
        }
    }

    /// Create a path from vertices and edges
    ///
    /// # Errors
    /// Returns an error if the path is invalid (wrong counts or discontinuous)
    pub fn from_parts(vertices: Vec<Vertex>, edges: Vec<Edge>) -> Result<Self, PathError> {
        let path = Self { vertices, edges };
        path.validate()?;
        Ok(path)
    }

    /// Validate the path invariants
    pub fn validate(&self) -> Result<(), PathError> {
        // Check empty
        if self.vertices.is_empty() {
            return Err(PathError::EmptyPath);
        }

        // Check count
        if self.vertices.len() != self.edges.len() + 1 {
            return Err(PathError::CountMismatch {
                vertices: self.vertices.len(),
                edges: self.edges.len(),
            });
        }

        // Check continuity
        for (i, edge) in self.edges.iter().enumerate() {
            let from_vertex = &self.vertices[i];
            let to_vertex = &self.vertices[i + 1];

            if edge.start != from_vertex.id {
                return Err(PathError::Discontinuity {
                    pos: i,
                    edge_start: edge.start,
                    edge_end: edge.end,
                    path_end: from_vertex.id,
                });
            }

            if edge.end != to_vertex.id {
                return Err(PathError::Discontinuity {
                    pos: i,
                    edge_start: edge.start,
                    edge_end: edge.end,
                    path_end: to_vertex.id,
                });
            }
        }

        Ok(())
    }

    /// Append an edge and vertex to the path
    ///
    /// # Errors
    /// Returns an error if the edge doesn't connect to the last vertex
    pub fn push(&mut self, edge: Edge, vertex: Vertex) -> Result<(), PathError> {
        let last_vertex = self.vertices.last().unwrap();

        if edge.start != last_vertex.id {
            return Err(PathError::Discontinuity {
                pos: self.edges.len(),
                edge_start: edge.start,
                edge_end: edge.end,
                path_end: last_vertex.id,
            });
        }

        if edge.end != vertex.id {
            return Err(PathError::Discontinuity {
                pos: self.edges.len(),
                edge_start: edge.start,
                edge_end: edge.end,
                path_end: vertex.id,
            });
        }

        self.edges.push(edge);
        self.vertices.push(vertex);
        Ok(())
    }

    /// Get the start vertex of the path
    pub fn start(&self) -> Option<&Vertex> {
        self.vertices.first()
    }

    /// Get the end vertex of the path
    pub fn end(&self) -> Option<&Vertex> {
        self.vertices.last()
    }

    /// Get the length of the path (number of edges)
    pub fn len(&self) -> usize {
        self.edges.len()
    }

    /// Check if the path is empty (only has a single vertex)
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    /// Get all vertex IDs in the path
    pub fn vertex_ids(&self) -> Vec<Graphid> {
        self.vertices.iter().map(|v| v.id).collect()
    }

    /// Get all edge IDs in the path
    pub fn edge_ids(&self) -> Vec<Graphid> {
        self.edges.iter().map(|e| e.id).collect()
    }

    /// Check if the path contains a vertex
    pub fn contains_vertex(&self, id: Graphid) -> bool {
        self.vertices.iter().any(|v| v.id == id)
    }

    /// Check if the path contains an edge
    pub fn contains_edge(&self, id: Graphid) -> bool {
        self.edges.iter().any(|e| e.id == id)
    }

    /// Reverse the path
    pub fn reverse(&self) -> Self {
        let vertices: Vec<Vertex> = self.vertices.iter().rev().cloned().collect();
        let edges: Vec<Edge> = self.edges.iter().rev().map(|e| e.reverse()).collect();

        Self { vertices, edges }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_test_vertex(labid: u16, locid: u64, name: &str) -> Vertex {
        let id = Graphid::new(labid, locid).unwrap();
        Vertex::new(id, "Person", json!({"name": name}))
    }

    fn create_test_edge(labid: u16, locid: u64, start: Graphid, end: Graphid) -> Edge {
        let id = Graphid::new(labid, locid).unwrap();
        Edge::new_empty(id, start, end, "KNOWS")
    }

    #[test]
    fn test_path_single_vertex() {
        let v1 = create_test_vertex(1, 100, "Alice");
        let path = GraphPath::new(v1.clone());

        assert_eq!(path.len(), 0);
        assert!(path.is_empty());
        assert_eq!(path.start(), Some(&v1));
        assert_eq!(path.end(), Some(&v1));
        assert!(path.validate().is_ok());
    }

    #[test]
    fn test_path_two_vertices() {
        let v1 = create_test_vertex(1, 100, "Alice");
        let v2 = create_test_vertex(1, 101, "Bob");
        let e1 = create_test_edge(2, 200, v1.id, v2.id);

        let path = GraphPath::from_parts(vec![v1.clone(), v2.clone()], vec![e1]).unwrap();

        assert_eq!(path.len(), 1);
        assert!(!path.is_empty());
        assert_eq!(path.start(), Some(&v1));
        assert_eq!(path.end(), Some(&v2));
    }

    #[test]
    fn test_path_push() {
        let v1 = create_test_vertex(1, 100, "Alice");
        let v2 = create_test_vertex(1, 101, "Bob");
        let v3 = create_test_vertex(1, 102, "Carol");

        let e1 = create_test_edge(2, 200, v1.id, v2.id);
        let e2 = create_test_edge(2, 201, v2.id, v3.id);

        let mut path = GraphPath::new(v1);
        path.push(e1, v2).unwrap();
        path.push(e2, v3.clone()).unwrap();

        assert_eq!(path.len(), 2);
        assert_eq!(path.end(), Some(&v3));
    }

    #[test]
    fn test_path_discontinuous() {
        let v1 = create_test_vertex(1, 100, "Alice");
        let v2 = create_test_vertex(1, 101, "Bob");
        let v3 = create_test_vertex(1, 102, "Carol");

        // Edge connects v1 to v3, but path has v2 in between
        let e1 = create_test_edge(2, 200, v1.id, v3.id);

        let result = GraphPath::from_parts(vec![v1, v2], vec![e1]);
        assert!(result.is_err());
    }

    #[test]
    fn test_path_count_mismatch() {
        let v1 = create_test_vertex(1, 100, "Alice");
        let v2 = create_test_vertex(1, 101, "Bob");
        let v3 = create_test_vertex(1, 102, "Carol");

        let e1 = create_test_edge(2, 200, v1.id, v2.id);

        // 3 vertices but only 1 edge
        let result = GraphPath::from_parts(vec![v1, v2, v3], vec![e1]);
        assert!(result.is_err());
    }

    #[test]
    fn test_path_reverse() {
        let v1 = create_test_vertex(1, 100, "Alice");
        let v2 = create_test_vertex(1, 101, "Bob");
        let v3 = create_test_vertex(1, 102, "Carol");

        let e1 = create_test_edge(2, 200, v1.id, v2.id);
        let e2 = create_test_edge(2, 201, v2.id, v3.id);

        let path = GraphPath::from_parts(vec![v1.clone(), v2.clone(), v3.clone()], vec![e1, e2])
            .unwrap();

        let reversed = path.reverse();

        assert_eq!(reversed.start().unwrap().id, v3.id);
        assert_eq!(reversed.end().unwrap().id, v1.id);
        assert_eq!(reversed.len(), 2);
        assert!(reversed.validate().is_ok());
    }

    #[test]
    fn test_path_contains() {
        let v1 = create_test_vertex(1, 100, "Alice");
        let v2 = create_test_vertex(1, 101, "Bob");
        let v3 = create_test_vertex(1, 102, "Carol");

        let e1 = create_test_edge(2, 200, v1.id, v2.id);

        let path = GraphPath::from_parts(vec![v1.clone(), v2.clone()], vec![e1.clone()]).unwrap();

        assert!(path.contains_vertex(v1.id));
        assert!(path.contains_vertex(v2.id));
        assert!(!path.contains_vertex(v3.id));

        assert!(path.contains_edge(e1.id));
    }
}
