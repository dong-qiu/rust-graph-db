/// Parallel graph algorithms
///
/// This module provides parallel versions of common graph operations
/// using rayon for multi-threaded execution.

use crate::storage::GraphStorage;
use crate::types::{Edge, Graphid, Vertex};
use rayon::prelude::*;
use std::sync::Arc;

/// Parallel filter function for vertices
///
/// Filters vertices in parallel using a predicate function.
///
/// # Arguments
/// * `vertices` - The vertices to filter
/// * `predicate` - A function that returns true for vertices to keep
///
/// # Returns
/// * A vector of vertices that match the predicate
pub fn parallel_filter_vertices<F>(vertices: Vec<Vertex>, predicate: F) -> Vec<Vertex>
where
    F: Fn(&Vertex) -> bool + Sync,
{
    vertices
        .into_par_iter()
        .filter(|v| predicate(v))
        .collect()
}

/// Parallel filter function for edges
///
/// Filters edges in parallel using a predicate function.
///
/// # Arguments
/// * `edges` - The edges to filter
/// * `predicate` - A function that returns true for edges to keep
///
/// # Returns
/// * A vector of edges that match the predicate
pub fn parallel_filter_edges<F>(edges: Vec<Edge>, predicate: F) -> Vec<Edge>
where
    F: Fn(&Edge) -> bool + Sync,
{
    edges.into_par_iter().filter(|e| predicate(e)).collect()
}

/// Parallel map function for vertices
///
/// Transforms vertices in parallel using a map function.
///
/// # Arguments
/// * `vertices` - The vertices to transform
/// * `transform` - A function to apply to each vertex
///
/// # Returns
/// * A vector of transformed results
pub fn parallel_map_vertices<T, F>(vertices: Vec<Vertex>, transform: F) -> Vec<T>
where
    T: Send,
    F: Fn(Vertex) -> T + Sync + Send,
{
    vertices.into_par_iter().map(transform).collect()
}

/// Parallel neighbor expansion
///
/// Expands neighbors from multiple source vertices in parallel.
///
/// # Arguments
/// * `storage` - The graph storage
/// * `source_ids` - The source vertex IDs
/// * `direction` - Whether to follow outgoing (true) or incoming (false) edges
///
/// # Returns
/// * A vector of (source_id, edges) tuples
pub async fn parallel_expand_neighbors(
    storage: Arc<dyn GraphStorage>,
    source_ids: Vec<Graphid>,
    outgoing: bool,
) -> Vec<(Graphid, Vec<Edge>)> {
    // Note: We use blocking here since rayon doesn't directly support async
    // For better async parallelism, consider using futures::stream
    source_ids
        .into_par_iter()
        .filter_map(|id| {
            let storage = storage.clone();
            // Use tokio's handle to run async code in blocking context
            let result = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    if outgoing {
                        storage.get_outgoing_edges(id).await
                    } else {
                        storage.get_incoming_edges(id).await
                    }
                })
            });
            result.ok().map(|edges| (id, edges))
        })
        .collect()
}

/// Parallel property matching
///
/// Checks property conditions on vertices in parallel.
///
/// # Arguments
/// * `vertices` - The vertices to check
/// * `property_name` - The property name to check
/// * `expected_value` - The expected value
///
/// # Returns
/// * Vertices that match the property condition
pub fn parallel_match_property(
    vertices: Vec<Vertex>,
    property_name: &str,
    expected_value: &serde_json::Value,
) -> Vec<Vertex> {
    let property_name = property_name.to_string();
    let expected_value = expected_value.clone();

    vertices
        .into_par_iter()
        .filter(|v| v.properties.get(&property_name) == Some(&expected_value))
        .collect()
}

/// Parallel batch processing
///
/// Process items in batches with parallel execution.
///
/// # Arguments
/// * `items` - Items to process
/// * `batch_size` - Size of each batch
/// * `processor` - Function to process each batch
///
/// # Returns
/// * Flattened results from all batches
pub fn parallel_batch_process<T, R, F>(items: Vec<T>, batch_size: usize, processor: F) -> Vec<R>
where
    T: Send + Clone,
    R: Send,
    F: Fn(Vec<T>) -> Vec<R> + Sync + Send,
{
    let batches: Vec<Vec<T>> = items
        .chunks(batch_size)
        .map(|chunk| chunk.to_vec())
        .collect();

    batches
        .into_par_iter()
        .flat_map(|batch| processor(batch))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Graphid;

    #[test]
    fn test_parallel_filter_vertices() {
        let vertices: Vec<Vertex> = (0..1000)
            .map(|i| Vertex {
                id: Graphid::new(1, i).unwrap(),
                label: "Person".to_string(),
                properties: serde_json::json!({"age": i}),
            })
            .collect();

        let filtered = parallel_filter_vertices(vertices, |v| {
            v.properties.get("age").and_then(|a| a.as_u64()).unwrap_or(0) > 500
        });

        assert_eq!(filtered.len(), 499);
    }

    #[test]
    fn test_parallel_map_vertices() {
        let vertices: Vec<Vertex> = (0..100)
            .map(|i| Vertex {
                id: Graphid::new(1, i).unwrap(),
                label: "Person".to_string(),
                properties: serde_json::json!({"name": format!("Person{}", i)}),
            })
            .collect();

        let ids: Vec<Graphid> = parallel_map_vertices(vertices, |v| v.id);
        assert_eq!(ids.len(), 100);
    }

    #[test]
    fn test_parallel_match_property() {
        let vertices: Vec<Vertex> = (0..100)
            .map(|i| Vertex {
                id: Graphid::new(1, i).unwrap(),
                label: "Person".to_string(),
                properties: serde_json::json!({"city": if i % 2 == 0 { "Beijing" } else { "Shanghai" }}),
            })
            .collect();

        let beijing_people =
            parallel_match_property(vertices, "city", &serde_json::json!("Beijing"));
        assert_eq!(beijing_people.len(), 50);
    }
}
