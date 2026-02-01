/// Shortest path algorithms
///
/// Implements Dijkstra's algorithm for finding shortest paths in the graph.

use super::{AlgorithmError, AlgorithmResult};
use crate::storage::GraphStorage;
use crate::types::{Edge, Graphid};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::sync::Arc;

/// Result of shortest path computation
#[derive(Debug, Clone, PartialEq)]
pub struct ShortestPathResult {
    /// Path as a sequence of vertex IDs
    pub path: Vec<Graphid>,
    /// Total cost/distance
    pub cost: u64,
    /// Edges traversed (in order)
    pub edges: Vec<Edge>,
}

/// Node in priority queue for Dijkstra's algorithm
#[derive(Debug, Clone, Eq, PartialEq)]
struct DijkstraNode {
    vertex: Graphid,
    cost: u64,
}

impl Ord for DijkstraNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap
        other.cost.cmp(&self.cost).then_with(|| self.vertex.cmp(&other.vertex))
    }
}

impl PartialOrd for DijkstraNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Find shortest path using Dijkstra's algorithm
///
/// # Arguments
/// * `storage` - Graph storage
/// * `start` - Start vertex ID
/// * `end` - End vertex ID
///
/// # Returns
/// * `Ok(ShortestPathResult)` - Shortest path found
/// * `Err(AlgorithmError::PathNotFound)` - No path exists
pub async fn shortest_path(
    storage: Arc<dyn GraphStorage>,
    start: Graphid,
    end: Graphid,
) -> AlgorithmResult<ShortestPathResult> {
    // Verify start and end vertices exist
    if storage.get_vertex(start).await?.is_none() {
        return Err(AlgorithmError::InvalidParameters(format!(
            "Start vertex {:?} not found",
            start
        )));
    }
    if storage.get_vertex(end).await?.is_none() {
        return Err(AlgorithmError::InvalidParameters(format!(
            "End vertex {:?} not found",
            end
        )));
    }

    // Run Dijkstra's algorithm
    dijkstra(storage, start, end).await
}

/// Dijkstra's algorithm implementation
pub async fn dijkstra(
    storage: Arc<dyn GraphStorage>,
    start: Graphid,
    end: Graphid,
) -> AlgorithmResult<ShortestPathResult> {
    let mut heap = BinaryHeap::new();
    let mut distances: HashMap<Graphid, u64> = HashMap::new();
    let mut predecessors: HashMap<Graphid, (Graphid, Edge)> = HashMap::new();
    let mut visited: HashSet<Graphid> = HashSet::new();

    // Initialize
    heap.push(DijkstraNode {
        vertex: start,
        cost: 0,
    });
    distances.insert(start, 0);

    while let Some(DijkstraNode { vertex, cost }) = heap.pop() {
        // Skip if already visited
        if visited.contains(&vertex) {
            continue;
        }

        // Mark as visited
        visited.insert(vertex);

        // Found destination
        if vertex == end {
            return Ok(reconstruct_path(start, end, &predecessors));
        }

        // Explore neighbors
        let edges = storage.get_outgoing_edges(vertex).await?;

        for edge in edges {
            let neighbor = edge.end;

            // Skip if already visited
            if visited.contains(&neighbor) {
                continue;
            }

            // Calculate new cost (uniform edge weight = 1)
            let new_cost = cost + 1;

            // Update if better path found
            let is_better = distances
                .get(&neighbor)
                .map(|&current| new_cost < current)
                .unwrap_or(true);

            if is_better {
                distances.insert(neighbor, new_cost);
                predecessors.insert(neighbor, (vertex, edge.clone()));
                heap.push(DijkstraNode {
                    vertex: neighbor,
                    cost: new_cost,
                });
            }
        }
    }

    // No path found
    Err(AlgorithmError::PathNotFound(start, end))
}

/// Reconstruct path from predecessors map
fn reconstruct_path(
    start: Graphid,
    end: Graphid,
    predecessors: &HashMap<Graphid, (Graphid, Edge)>,
) -> ShortestPathResult {
    let mut path = Vec::new();
    let mut edges = Vec::new();
    let mut current = end;
    let mut cost = 0;

    // Walk backwards from end to start
    while current != start {
        path.push(current);

        if let Some((prev, edge)) = predecessors.get(&current) {
            edges.push(edge.clone());
            current = *prev;
            cost += 1;
        } else {
            break;
        }
    }

    path.push(start);

    // Reverse to get start -> end order
    path.reverse();
    edges.reverse();

    ShortestPathResult { path, cost, edges }
}

/// Find all shortest paths within max hops
///
/// # Arguments
/// * `storage` - Graph storage
/// * `start` - Start vertex ID
/// * `max_hops` - Maximum number of hops
///
/// # Returns
/// * Map from vertex ID to shortest path result
pub async fn shortest_paths_from(
    storage: Arc<dyn GraphStorage>,
    start: Graphid,
    max_hops: usize,
) -> AlgorithmResult<HashMap<Graphid, ShortestPathResult>> {
    let mut heap = BinaryHeap::new();
    let mut distances: HashMap<Graphid, u64> = HashMap::new();
    let mut predecessors: HashMap<Graphid, (Graphid, Edge)> = HashMap::new();
    let mut visited: HashSet<Graphid> = HashSet::new();
    let mut results: HashMap<Graphid, ShortestPathResult> = HashMap::new();

    // Initialize
    heap.push(DijkstraNode {
        vertex: start,
        cost: 0,
    });
    distances.insert(start, 0);

    while let Some(DijkstraNode { vertex, cost }) = heap.pop() {
        // Skip if already visited
        if visited.contains(&vertex) {
            continue;
        }

        // Mark as visited
        visited.insert(vertex);

        // Add to results (except start itself)
        if vertex != start {
            results.insert(vertex, reconstruct_path(start, vertex, &predecessors));
        }

        // Stop if reached max hops
        if cost >= max_hops as u64 {
            continue;
        }

        // Explore neighbors
        let edges = storage.get_outgoing_edges(vertex).await?;

        for edge in edges {
            let neighbor = edge.end;

            // Skip if already visited
            if visited.contains(&neighbor) {
                continue;
            }

            // Calculate new cost
            let new_cost = cost + 1;

            // Update if better path found
            let is_better = distances
                .get(&neighbor)
                .map(|&current| new_cost < current)
                .unwrap_or(true);

            if is_better {
                distances.insert(neighbor, new_cost);
                predecessors.insert(neighbor, (vertex, edge.clone()));
                heap.push(DijkstraNode {
                    vertex: neighbor,
                    cost: new_cost,
                });
            }
        }
    }

    Ok(results)
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
    async fn test_shortest_path_direct() {
        let (storage, _temp) = setup_test_graph().await;

        // Get vertices
        let vertices = storage.scan_vertices("Node").await.unwrap();
        let a = vertices.iter().find(|v| v.properties["name"] == "A").unwrap();
        let b = vertices.iter().find(|v| v.properties["name"] == "B").unwrap();

        // Find shortest path A -> B
        let result = shortest_path(storage.clone(), a.id, b.id).await.unwrap();

        assert_eq!(result.path.len(), 2);
        assert_eq!(result.cost, 1);
        assert_eq!(result.edges.len(), 1);
        assert_eq!(result.path[0], a.id);
        assert_eq!(result.path[1], b.id);
    }

    #[tokio::test]
    async fn test_shortest_path_multiple_hops() {
        let (storage, _temp) = setup_test_graph().await;

        // Get vertices
        let vertices = storage.scan_vertices("Node").await.unwrap();
        let a = vertices.iter().find(|v| v.properties["name"] == "A").unwrap();
        let d = vertices.iter().find(|v| v.properties["name"] == "D").unwrap();

        // Find shortest path A -> D
        let result = shortest_path(storage.clone(), a.id, d.id).await.unwrap();

        assert_eq!(result.path.len(), 3);
        assert_eq!(result.cost, 2);
        assert_eq!(result.edges.len(), 2);
        assert_eq!(result.path[0], a.id);
        assert_eq!(result.path[2], d.id);
    }

    #[tokio::test]
    async fn test_shortest_path_not_found() {
        let (storage, _temp) = setup_test_graph().await;

        // Get vertices
        let vertices = storage.scan_vertices("Node").await.unwrap();
        let d = vertices.iter().find(|v| v.properties["name"] == "D").unwrap();
        let a = vertices.iter().find(|v| v.properties["name"] == "A").unwrap();

        // Try to find path D -> A (no such path)
        let result = shortest_path(storage.clone(), d.id, a.id).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AlgorithmError::PathNotFound(_, _)
        ));
    }

    #[tokio::test]
    async fn test_shortest_paths_from() {
        let (storage, _temp) = setup_test_graph().await;

        // Get vertex A
        let vertices = storage.scan_vertices("Node").await.unwrap();
        let a = vertices.iter().find(|v| v.properties["name"] == "A").unwrap();

        // Find all shortest paths from A within 2 hops
        let results = shortest_paths_from(storage.clone(), a.id, 2).await.unwrap();

        // Should reach B (1 hop), C (1 hop), D (2 hops), E (2 hops)
        assert_eq!(results.len(), 4);

        // Check costs
        assert!(results.values().any(|r| r.cost == 1)); // B or C
        assert!(results.values().any(|r| r.cost == 2)); // D or E
    }
}
