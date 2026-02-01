/// Graph algorithms demonstration
///
/// This example shows how to use graph algorithms:
/// - Shortest path (Dijkstra)
/// - Variable-length expansion (VLE)
/// - K-hop neighbors

use rust_graph_db::storage::rocksdb_store::RocksDbStorage;
use rust_graph_db::storage::GraphStorage;
use rust_graph_db::{shortest_path, variable_length_expand, VleOptions};
use rust_graph_db::algorithms::vle::{k_hop_neighbors, neighbors_within_k_hops};
use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Graph Algorithms Demo ===\n");

    // Setup storage and create sample graph
    let temp_dir = TempDir::new()?;
    let storage = Arc::new(RocksDbStorage::new(temp_dir.path(), "demo_graph")?);

    println!("1. Creating sample graph...");
    create_sample_graph(storage.clone()).await?;
    println!("   Graph created: A -> B -> D");
    println!("                  |    |");
    println!("                  v    v");
    println!("                  C -> E\n");

    // Get vertex IDs
    let vertices = storage.scan_vertices("City").await?;
    let get_vertex_by_name = |name: &str| {
        vertices
            .iter()
            .find(|v| v.properties["name"] == name)
            .unwrap()
    };

    let a = get_vertex_by_name("A");
    let b = get_vertex_by_name("B");
    let c = get_vertex_by_name("C");
    let d = get_vertex_by_name("D");
    let e = get_vertex_by_name("E");

    // Example 1: Shortest path
    println!("2. Finding shortest path from A to D...");
    let result = shortest_path(storage.clone(), a.id, d.id).await?;
    println!("   Path found:");
    println!("   - Length: {} hops", result.cost);
    println!("   - Vertices: {} nodes", result.path.len());
    println!("   - Edges: {}", result.edges.len());

    let path_names: Vec<String> = result
        .path
        .iter()
        .map(|&id| {
            let v = vertices.iter().find(|v| v.id == id).unwrap();
            v.properties["name"].as_str().unwrap().to_string()
        })
        .collect();
    println!("   - Route: {}\n", path_names.join(" -> "));

    // Example 2: Variable-length expansion
    println!("3. Finding all paths from A (1-2 hops)...");
    let vle_options = VleOptions {
        min_length: 1,
        max_length: 2,
        allow_cycles: false,
        max_paths: 0,
    };

    let paths = variable_length_expand(storage.clone(), a.id, vle_options).await?;
    println!("   Found {} paths:", paths.len());

    for (i, path) in paths.iter().enumerate() {
        let path_names: Vec<String> = path
            .vertices
            .iter()
            .map(|&id| {
                let v = vertices.iter().find(|v| v.id == id).unwrap();
                v.properties["name"].as_str().unwrap().to_string()
            })
            .collect();
        println!("   {}. {} (length: {})", i + 1, path_names.join(" -> "), path.length);
    }
    println!();

    // Example 3: K-hop neighbors
    println!("4. Finding 1-hop neighbors of A...");
    let neighbors = k_hop_neighbors(storage.clone(), a.id, 1).await?;
    println!("   Found {} neighbors:", neighbors.len());

    for neighbor_id in &neighbors {
        let v = vertices.iter().find(|v| v.id == *neighbor_id).unwrap();
        println!("   - {}", v.properties["name"]);
    }
    println!();

    // Example 4: Neighbors within k hops
    println!("5. Finding all neighbors within 2 hops of A...");
    let all_neighbors = neighbors_within_k_hops(storage.clone(), a.id, 2).await?;
    println!("   Found {} reachable vertices:", all_neighbors.len());

    for neighbor_id in &all_neighbors {
        let v = vertices.iter().find(|v| v.id == *neighbor_id).unwrap();
        println!("   - {}", v.properties["name"]);
    }
    println!();

    // Example 5: All paths between two vertices
    println!("6. Finding all 2-hop paths from A to E...");
    use rust_graph_db::algorithms::vle::variable_length_paths_between;

    let vle_options = VleOptions {
        min_length: 2,
        max_length: 2,
        allow_cycles: false,
        max_paths: 0,
    };

    let paths_ae = variable_length_paths_between(storage.clone(), a.id, e.id, vle_options).await?;
    println!("   Found {} paths:", paths_ae.len());

    for (i, path) in paths_ae.iter().enumerate() {
        let path_names: Vec<String> = path
            .vertices
            .iter()
            .map(|&id| {
                let v = vertices.iter().find(|v| v.id == id).unwrap();
                v.properties["name"].as_str().unwrap().to_string()
            })
            .collect();
        println!("   {}. {}", i + 1, path_names.join(" -> "));
    }
    println!();

    println!("=== Demo Complete ===");

    Ok(())
}

async fn create_sample_graph(
    storage: Arc<RocksDbStorage>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut tx = storage.begin_transaction().await?;

    // Create vertices
    let a = tx.create_vertex("City", json!({"name": "A"})).await?;
    let b = tx.create_vertex("City", json!({"name": "B"})).await?;
    let c = tx.create_vertex("City", json!({"name": "C"})).await?;
    let d = tx.create_vertex("City", json!({"name": "D"})).await?;
    let e = tx.create_vertex("City", json!({"name": "E"})).await?;

    // Create edges
    tx.create_edge("ROAD", a.id, b.id, json!({})).await?;
    tx.create_edge("ROAD", a.id, c.id, json!({})).await?;
    tx.create_edge("ROAD", b.id, d.id, json!({})).await?;
    tx.create_edge("ROAD", b.id, e.id, json!({})).await?;
    tx.create_edge("ROAD", c.id, e.id, json!({})).await?;

    tx.commit().await?;

    Ok(())
}
