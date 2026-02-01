/// Basic usage example for the Rust Graph Database
///
/// This example demonstrates:
/// 1. Creating vertices (nodes)
/// 2. Creating edges (relationships)
/// 3. Constructing graph paths
/// 4. Working with properties

use rust_graph_db::{Edge, Graphid, GraphPath, Vertex};
use serde_json::json;

fn main() {
    println!("=== Rust Graph Database - Basic Usage Example ===\n");

    // Example 1: Creating Vertices
    println!("1. Creating Vertices");
    println!("{}", "-".repeat(50));

    let alice_id = Graphid::new(1, 100).expect("Failed to create Graphid");
    let alice = Vertex::new(
        alice_id,
        "Person",
        json!({
            "name": "Alice",
            "age": 30,
            "city": "Beijing"
        }),
    );

    let bob_id = Graphid::new(1, 101).expect("Failed to create Graphid");
    let bob = Vertex::new(
        bob_id,
        "Person",
        json!({
            "name": "Bob",
            "age": 25,
            "city": "Shanghai"
        }),
    );

    let carol_id = Graphid::new(1, 102).expect("Failed to create Graphid");
    let carol = Vertex::new(
        carol_id,
        "Person",
        json!({
            "name": "Carol",
            "age": 28,
            "city": "Shenzhen"
        }),
    );

    println!("Created vertex: {:?}", alice.id);
    println!("  Label: {}", alice.label);
    println!("  Name: {}", alice.get_property("name").unwrap());
    println!("  Age: {}\n", alice.get_property("age").unwrap());

    // Example 2: Creating Edges
    println!("2. Creating Edges (Relationships)");
    println!("{}", "-".repeat(50));

    let edge1_id = Graphid::new(2, 200).expect("Failed to create edge Graphid");
    let knows_edge = Edge::new(
        edge1_id,
        alice_id,
        bob_id,
        "KNOWS",
        json!({
            "since": 2020,
            "relationship": "friend"
        }),
    );

    let edge2_id = Graphid::new(2, 201).expect("Failed to create edge Graphid");
    let works_with_edge = Edge::new(
        edge2_id,
        bob_id,
        carol_id,
        "WORKS_WITH",
        json!({
            "since": 2021,
            "department": "Engineering"
        }),
    );

    println!("Created edge: {:?}", knows_edge.id);
    println!("  Type: {}", knows_edge.label);
    println!("  From: {:?}", knows_edge.start);
    println!("  To: {:?}", knows_edge.end);
    println!(
        "  Since: {}\n",
        knows_edge.get_property("since").unwrap()
    );

    // Example 3: Creating a Graph Path
    println!("3. Creating a Graph Path");
    println!("{}", "-".repeat(50));

    let path = GraphPath::from_parts(
        vec![alice.clone(), bob.clone(), carol.clone()],
        vec![knows_edge.clone(), works_with_edge.clone()],
    )
    .expect("Failed to create path");

    println!("Path created:");
    println!("  Start: {} ({})",
        path.start().unwrap().get_property("name").unwrap(),
        path.start().unwrap().id
    );
    println!("  End: {} ({})",
        path.end().unwrap().get_property("name").unwrap(),
        path.end().unwrap().id
    );
    println!("  Length: {} edges", path.len());
    println!("  Vertex IDs: {:?}\n", path.vertex_ids());

    // Example 4: Working with Properties
    println!("4. Working with Properties");
    println!("{}", "-".repeat(50));

    let mut person = Vertex::new_empty(Graphid::new(1, 103).unwrap(), "Person");

    println!("Initial properties: {:?}", person.property_keys());

    person.set_property("name", json!("David"));
    person.set_property("age", json!(35));
    person.set_property("city", json!("Guangzhou"));

    println!("After adding properties: {:?}", person.property_keys());
    println!("  Name: {}", person.get_property("name").unwrap());

    person.remove_property("age");
    println!(
        "After removing 'age': {:?}\n",
        person.property_keys()
    );

    // Example 5: Graphid Operations
    println!("5. Graphid Operations");
    println!("{}", "-".repeat(50));

    let id = Graphid::new(10, 123456).unwrap();
    println!("Graphid: {}", id);
    println!("  Label ID: {}", id.labid());
    println!("  Local ID: {}", id.locid());
    println!("  Raw value: {:#x}\n", id.as_raw());

    // Example 6: Edge Operations
    println!("6. Edge Operations");
    println!("{}", "-".repeat(50));

    let self_loop = Edge::new_empty(
        Graphid::new(2, 300).unwrap(),
        alice_id,
        alice_id,
        "LIKES",
    );

    println!("Is self-loop: {}", self_loop.is_self_loop());

    let reversed = knows_edge.reverse();
    println!("Original edge: {} -> {}", knows_edge.start, knows_edge.end);
    println!("Reversed edge: {} -> {}\n", reversed.start, reversed.end);

    // Example 7: Path Validation
    println!("7. Path Validation");
    println!("{}", "-".repeat(50));

    match path.validate() {
        Ok(_) => println!("✓ Path is valid"),
        Err(e) => println!("✗ Path error: {}", e),
    }

    println!("Path contains vertex {}: {}",
        alice_id,
        path.contains_vertex(alice_id)
    );
    println!("Path contains edge {}: {}\n",
        edge1_id,
        path.contains_edge(edge1_id)
    );

    // Example 8: JSON Serialization
    println!("8. JSON Serialization");
    println!("{}", "-".repeat(50));

    let json_str = serde_json::to_string_pretty(&alice).unwrap();
    println!("Vertex as JSON:\n{}\n", json_str);

    let deserialized: Vertex = serde_json::from_str(&json_str).unwrap();
    println!("Deserialized name: {}",
        deserialized.get_property("name").unwrap()
    );

    println!("\n=== Example Complete ===");
}
