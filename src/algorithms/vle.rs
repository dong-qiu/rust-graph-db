/// Variable Length Expansion (VLE)
///
/// Implements variable-length path expansion for Cypher queries.

use super::{AlgorithmError, AlgorithmResult};
use crate::storage::GraphStorage;
use crate::types::{Edge, Graphid};
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

/// A variable-length path
#[derive(Debug, Clone, PartialEq)]
pub struct VariableLengthPath {
    /// Vertices in the path (in order)
    pub vertices: Vec<Graphid>,
    /// Edges in the path (in order)
    pub edges: Vec<Edge>,
    /// Length of the path (number of edges)
    pub length: usize,
}

impl VariableLengthPath {
    /// Create a new path starting with a single vertex
    pub fn start_from(vertex: Graphid) -> Self {
        Self {
            vertices: vec![vertex],
            edges: Vec::new(),
            length: 0,
        }
    }

    /// Extend path with an edge and vertex
    pub fn extend(&self, edge: Edge, vertex: Graphid) -> Self {
        let mut new_path = self.clone();
        new_path.edges.push(edge);
        new_path.vertices.push(vertex);
        new_path.length += 1;
        new_path
    }

    /// Check if path contains a vertex (for cycle detection)
    pub fn contains_vertex(&self, vertex: Graphid) -> bool {
        self.vertices.contains(&vertex)
    }

    /// Get the last vertex in the path
    pub fn last_vertex(&self) -> Graphid {
        *self.vertices.last().unwrap()
    }
}

/// Variable length expansion options
#[derive(Debug, Clone)]
pub struct VleOptions {
    /// Minimum path length
    pub min_length: usize,
    /// Maximum path length
    pub max_length: usize,
    /// Allow cycles in paths
    pub allow_cycles: bool,
    /// Maximum number of paths to return (0 = unlimited)
    pub max_paths: usize,
}

impl Default for VleOptions {
    fn default() -> Self {
        Self {
            min_length: 1,
            max_length: 5,
            allow_cycles: false,
            max_paths: 0,
        }
    }
}

/// Find all variable-length paths from start vertex
///
/// # Arguments
/// * `storage` - Graph storage
/// * `start` - Start vertex ID
/// * `options` - VLE options
///
/// # Returns
/// * Vector of all paths matching criteria
pub async fn variable_length_expand(
    storage: Arc<dyn GraphStorage>,
    start: Graphid,
    options: VleOptions,
) -> AlgorithmResult<Vec<VariableLengthPath>> {
    // Validate options
    if options.min_length > options.max_length {
        return Err(AlgorithmError::InvalidParameters(
            "min_length cannot be greater than max_length".to_string(),
        ));
    }

    if options.max_length == 0 {
        return Err(AlgorithmError::InvalidParameters(
            "max_length must be at least 1".to_string(),
        ));
    }

    // Verify start vertex exists
    if storage.get_vertex(start).await?.is_none() {
        return Err(AlgorithmError::InvalidParameters(format!(
            "Start vertex {:?} not found",
            start
        )));
    }

    let mut results = Vec::new();
    let mut queue = VecDeque::new();

    // Initialize with start vertex
    queue.push_back(VariableLengthPath::start_from(start));

    while let Some(path) = queue.pop_front() {
        let current_length = path.length;

        // Add to results if within valid range
        if current_length >= options.min_length {
            results.push(path.clone());

            // Check if we've reached max paths limit
            if options.max_paths > 0 && results.len() >= options.max_paths {
                break;
            }
        }

        // Continue expanding if not at max length
        if current_length < options.max_length {
            let current_vertex = path.last_vertex();

            // Get outgoing edges
            let edges = storage.get_outgoing_edges(current_vertex).await?;

            for edge in edges {
                let next_vertex = edge.end;

                // Check for cycles
                if !options.allow_cycles && path.contains_vertex(next_vertex) {
                    continue;
                }

                // Extend path
                let new_path = path.extend(edge, next_vertex);
                queue.push_back(new_path);
            }
        }
    }

    Ok(results)
}

/// Find all variable-length paths between two vertices
///
/// # Arguments
/// * `storage` - Graph storage
/// * `start` - Start vertex ID
/// * `end` - End vertex ID
/// * `options` - VLE options
///
/// # Returns
/// * Vector of all paths from start to end matching criteria
pub async fn variable_length_paths_between(
    storage: Arc<dyn GraphStorage>,
    start: Graphid,
    end: Graphid,
    options: VleOptions,
) -> AlgorithmResult<Vec<VariableLengthPath>> {
    // Get all paths from start
    let all_paths = variable_length_expand(storage, start, options).await?;

    // Filter paths that end at target vertex
    let filtered_paths: Vec<VariableLengthPath> = all_paths
        .into_iter()
        .filter(|path| path.last_vertex() == end)
        .collect();

    if filtered_paths.is_empty() {
        return Err(AlgorithmError::PathNotFound(start, end));
    }

    Ok(filtered_paths)
}

/// Find k-hop neighbors
///
/// # Arguments
/// * `storage` - Graph storage
/// * `start` - Start vertex ID
/// * `k` - Number of hops
///
/// # Returns
/// * Set of vertex IDs reachable in exactly k hops
pub async fn k_hop_neighbors(
    storage: Arc<dyn GraphStorage>,
    start: Graphid,
    k: usize,
) -> AlgorithmResult<HashSet<Graphid>> {
    if k == 0 {
        return Ok(HashSet::from([start]));
    }

    let options = VleOptions {
        min_length: k,
        max_length: k,
        allow_cycles: false,
        max_paths: 0,
    };

    let paths = variable_length_expand(storage, start, options).await?;

    let neighbors: HashSet<Graphid> = paths
        .into_iter()
        .map(|path| path.last_vertex())
        .collect();

    Ok(neighbors)
}

/// Find all neighbors within k hops
///
/// # Arguments
/// * `storage` - Graph storage
/// * `start` - Start vertex ID
/// * `k` - Maximum number of hops
///
/// # Returns
/// * Set of vertex IDs reachable within k hops
pub async fn neighbors_within_k_hops(
    storage: Arc<dyn GraphStorage>,
    start: Graphid,
    k: usize,
) -> AlgorithmResult<HashSet<Graphid>> {
    let options = VleOptions {
        min_length: 1,
        max_length: k,
        allow_cycles: false,
        max_paths: 0,
    };

    let paths = variable_length_expand(storage, start, options).await?;

    let neighbors: HashSet<Graphid> = paths
        .into_iter()
        .map(|path| path.last_vertex())
        .collect();

    Ok(neighbors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::rocksdb_store::RocksDbStorage;
    use serde_json::json;
    use tempfile::TempDir;

    async fn setup_test_graph() -> (Arc<dyn GraphStorage>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage: Arc<dyn GraphStorage> =
            Arc::new(RocksDbStorage::new(temp_dir.path(), "test_graph").unwrap());

        // Create test graph:
        //   A -> B -> D
        //   |    |
        //   v    v
        //   C -> E
        let mut tx = storage.begin_transaction().await.unwrap();

        let a = tx
            .create_vertex("Node", json!({"name": "A"}))
            .await
            .unwrap();
        let b = tx
            .create_vertex("Node", json!({"name": "B"}))
            .await
            .unwrap();
        let c = tx
            .create_vertex("Node", json!({"name": "C"}))
            .await
            .unwrap();
        let d = tx
            .create_vertex("Node", json!({"name": "D"}))
            .await
            .unwrap();
        let e = tx
            .create_vertex("Node", json!({"name": "E"}))
            .await
            .unwrap();

        // Edges
        tx.create_edge("LINK", a.id, b.id, json!({}))
            .await
            .unwrap();
        tx.create_edge("LINK", a.id, c.id, json!({}))
            .await
            .unwrap();
        tx.create_edge("LINK", b.id, d.id, json!({}))
            .await
            .unwrap();
        tx.create_edge("LINK", b.id, e.id, json!({}))
            .await
            .unwrap();
        tx.create_edge("LINK", c.id, e.id, json!({}))
            .await
            .unwrap();

        tx.commit().await.unwrap();

        (storage, temp_dir)
    }

    #[tokio::test]
    async fn test_vle_basic() {
        let (storage, _temp) = setup_test_graph().await;

        // Get vertex A
        let vertices = storage.scan_vertices("Node").await.unwrap();
        let a = vertices.iter().find(|v| v.properties["name"] == "A").unwrap();

        // Find all paths from A with length 1-2
        let options = VleOptions {
            min_length: 1,
            max_length: 2,
            allow_cycles: false,
            max_paths: 0,
        };

        let paths = variable_length_expand(storage.clone(), a.id, options)
            .await
            .unwrap();

        // Should find:
        // - 1-hop: A->B, A->C
        // - 2-hop: A->B->D, A->B->E, A->C->E
        assert!(paths.len() >= 5);

        // Check that all paths start from A
        assert!(paths.iter().all(|p| p.vertices[0] == a.id));

        // Check path lengths
        assert!(paths.iter().any(|p| p.length == 1));
        assert!(paths.iter().any(|p| p.length == 2));
    }

    #[tokio::test]
    async fn test_vle_paths_between() {
        let (storage, _temp) = setup_test_graph().await;

        // Get vertices
        let vertices = storage.scan_vertices("Node").await.unwrap();
        let a = vertices.iter().find(|v| v.properties["name"] == "A").unwrap();
        let e = vertices.iter().find(|v| v.properties["name"] == "E").unwrap();

        // Find all paths from A to E with length 2
        let options = VleOptions {
            min_length: 2,
            max_length: 2,
            allow_cycles: false,
            max_paths: 0,
        };

        let paths = variable_length_paths_between(storage.clone(), a.id, e.id, options)
            .await
            .unwrap();

        // Should find 2 paths: A->B->E and A->C->E
        assert_eq!(paths.len(), 2);

        // All paths should be length 2
        assert!(paths.iter().all(|p| p.length == 2));

        // All paths should start at A and end at E
        assert!(paths.iter().all(|p| p.vertices[0] == a.id));
        assert!(paths.iter().all(|p| p.last_vertex() == e.id));
    }

    #[tokio::test]
    async fn test_k_hop_neighbors() {
        let (storage, _temp) = setup_test_graph().await;

        // Get vertex A
        let vertices = storage.scan_vertices("Node").await.unwrap();
        let a = vertices.iter().find(|v| v.properties["name"] == "A").unwrap();
        let b = vertices.iter().find(|v| v.properties["name"] == "B").unwrap();
        let c = vertices.iter().find(|v| v.properties["name"] == "C").unwrap();

        // Find 1-hop neighbors of A
        let neighbors = k_hop_neighbors(storage.clone(), a.id, 1)
            .await
            .unwrap();

        // Should be B and C
        assert_eq!(neighbors.len(), 2);
        assert!(neighbors.contains(&b.id));
        assert!(neighbors.contains(&c.id));
    }

    #[tokio::test]
    async fn test_neighbors_within_k_hops() {
        let (storage, _temp) = setup_test_graph().await;

        // Get vertex A
        let vertices = storage.scan_vertices("Node").await.unwrap();
        let a = vertices.iter().find(|v| v.properties["name"] == "A").unwrap();

        // Find all neighbors within 2 hops of A
        let neighbors = neighbors_within_k_hops(storage.clone(), a.id, 2)
            .await
            .unwrap();

        // Should reach B, C (1-hop) and D, E (2-hops)
        assert_eq!(neighbors.len(), 4);
    }

    #[tokio::test]
    async fn test_vle_max_paths_limit() {
        let (storage, _temp) = setup_test_graph().await;

        // Get vertex A
        let vertices = storage.scan_vertices("Node").await.unwrap();
        let a = vertices.iter().find(|v| v.properties["name"] == "A").unwrap();

        // Find paths with max_paths limit
        let options = VleOptions {
            min_length: 1,
            max_length: 2,
            allow_cycles: false,
            max_paths: 3,
        };

        let paths = variable_length_expand(storage.clone(), a.id, options)
            .await
            .unwrap();

        // Should return at most 3 paths
        assert!(paths.len() <= 3);
    }
}
