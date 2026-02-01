use clap::{Parser, ValueEnum};
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufWriter, Write as IoWrite};
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};

#[derive(Debug, Clone, ValueEnum)]
enum GraphType {
    Uniform,
    PowerLaw,
    Grid,
    Tree,
}

#[derive(Parser, Debug)]
#[command(name = "data_generator")]
#[command(about = "Generate graph datasets for benchmarking", long_about = None)]
struct Args {
    /// Type of graph to generate
    #[arg(short, long, value_enum)]
    graph_type: GraphType,

    /// Number of vertices
    #[arg(short = 'n', long, default_value_t = 1000)]
    vertices: usize,

    /// Average degree for uniform/power-law graphs
    #[arg(short = 'd', long, default_value_t = 10)]
    avg_degree: usize,

    /// Grid size (for grid graphs, creates size×size grid)
    #[arg(short = 's', long)]
    size: Option<usize>,

    /// Tree depth (for tree graphs)
    #[arg(long, default_value_t = 4)]
    depth: usize,

    /// Tree branching factor
    #[arg(short = 'b', long, default_value_t = 3)]
    branching: usize,

    /// Output directory
    #[arg(short, long)]
    output: PathBuf,

    /// Export formats (csv, json, cypher)
    #[arg(short, long, value_delimiter = ',', default_value = "json")]
    formats: Vec<String>,

    /// Random seed for reproducibility
    #[arg(long, default_value_t = 42)]
    seed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Vertex {
    id: usize,
    label: String,
    properties: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Edge {
    label: String,
    start: usize,
    end: usize,
    properties: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GraphData {
    vertices: Vec<Vertex>,
    edges: Vec<Edge>,
}

struct GraphGenerator {
    rng: StdRng,
}

impl GraphGenerator {
    fn new(seed: u64) -> Self {
        Self {
            rng: StdRng::seed_from_u64(seed),
        }
    }

    /// Generate uniform random graph (Erdős-Rényi-like with fixed average degree)
    fn generate_uniform(&mut self, n: usize, avg_degree: usize) -> GraphData {
        println!("Generating uniform random graph with {} vertices, avg degree {}...", n, avg_degree);

        let mut vertices = Vec::with_capacity(n);
        let cities = vec!["NYC", "LA", "Chicago", "Houston", "Phoenix", "Philadelphia",
                         "San Antonio", "San Diego", "Dallas", "San Jose"];
        let occupations = vec!["Engineer", "Teacher", "Doctor", "Artist", "Manager",
                              "Scientist", "Writer", "Designer", "Analyst", "Developer"];

        // Create vertices
        for i in 0..n {
            let mut props = HashMap::new();
            props.insert("name".to_string(), serde_json::json!(format!("Person{}", i)));
            props.insert("age".to_string(), serde_json::json!(self.rng.gen_range(18..80)));
            props.insert("city".to_string(), serde_json::json!(cities[self.rng.gen_range(0..cities.len())]));
            props.insert("occupation".to_string(), serde_json::json!(occupations[self.rng.gen_range(0..occupations.len())]));

            vertices.push(Vertex {
                id: i,
                label: "Person".to_string(),
                properties: props,
            });
        }

        // Create edges with target average degree
        let num_edges = (n * avg_degree) / 2;
        let mut edges = Vec::with_capacity(num_edges);
        let mut edge_set = HashSet::new();

        let mut attempts = 0;
        let max_attempts = num_edges * 10;

        while edges.len() < num_edges && attempts < max_attempts {
            let start = self.rng.gen_range(0..n);
            let end = self.rng.gen_range(0..n);

            if start != end && !edge_set.contains(&(start, end)) && !edge_set.contains(&(end, start)) {
                let mut props = HashMap::new();
                props.insert("since".to_string(), serde_json::json!(self.rng.gen_range(2000..2024)));
                props.insert("weight".to_string(), serde_json::json!(self.rng.gen_range(1.0..10.0)));

                edges.push(Edge {
                    label: "KNOWS".to_string(),
                    start,
                    end,
                    properties: props,
                });
                edge_set.insert((start, end));
            }
            attempts += 1;
        }

        println!("Generated {} vertices and {} edges", vertices.len(), edges.len());
        GraphData { vertices, edges }
    }

    /// Generate power-law graph using Barabási-Albert preferential attachment
    fn generate_power_law(&mut self, n: usize, m_per_node: usize) -> GraphData {
        println!("Generating power-law graph with {} vertices, {} edges per new node...", n, m_per_node);

        let mut vertices = Vec::with_capacity(n);
        let cities = vec!["NYC", "LA", "Chicago", "Houston", "Phoenix"];
        let occupations = vec!["Engineer", "Influencer", "Developer", "Artist", "Manager"];

        // Create initial fully connected graph
        let initial_size = m_per_node + 1;
        for i in 0..n {
            let mut props = HashMap::new();
            props.insert("name".to_string(), serde_json::json!(format!("User{}", i)));
            props.insert("age".to_string(), serde_json::json!(self.rng.gen_range(18..65)));
            props.insert("city".to_string(), serde_json::json!(cities[self.rng.gen_range(0..cities.len())]));
            props.insert("occupation".to_string(), serde_json::json!(occupations[self.rng.gen_range(0..occupations.len())]));

            vertices.push(Vertex {
                id: i,
                label: "User".to_string(),
                properties: props,
            });
        }

        let mut edges = Vec::new();
        let mut degree: HashMap<usize, usize> = HashMap::new();

        // Initialize with complete graph
        for i in 0..initial_size.min(n) {
            for j in (i + 1)..initial_size.min(n) {
                let mut props = HashMap::new();
                props.insert("since".to_string(), serde_json::json!(2015));

                edges.push(Edge {
                    label: "FOLLOWS".to_string(),
                    start: i,
                    end: j,
                    properties: props.clone(),
                });

                *degree.entry(i).or_insert(0) += 1;
                *degree.entry(j).or_insert(0) += 1;
            }
        }

        // Preferential attachment for remaining nodes
        for new_node in initial_size..n {
            let total_degree: usize = degree.values().sum();
            if total_degree == 0 {
                break;
            }

            let mut targets = HashSet::new();
            let mut attempts = 0;

            while targets.len() < m_per_node && attempts < 100 {
                let mut cumulative = 0.0;
                let threshold = self.rng.gen::<f64>() * total_degree as f64;

                for (&node, &deg) in degree.iter() {
                    cumulative += deg as f64;
                    if cumulative >= threshold && node != new_node && !targets.contains(&node) {
                        targets.insert(node);
                        break;
                    }
                }
                attempts += 1;
            }

            for &target in &targets {
                let mut props = HashMap::new();
                props.insert("since".to_string(), serde_json::json!(self.rng.gen_range(2015..2024)));

                edges.push(Edge {
                    label: "FOLLOWS".to_string(),
                    start: new_node,
                    end: target,
                    properties: props,
                });

                *degree.entry(new_node).or_insert(0) += 1;
                *degree.entry(target).or_insert(0) += 1;
            }
        }

        println!("Generated {} vertices and {} edges", vertices.len(), edges.len());
        GraphData { vertices, edges }
    }

    /// Generate grid graph (N×N 2D grid)
    fn generate_grid(&mut self, rows: usize, cols: usize) -> GraphData {
        println!("Generating {}×{} grid graph...", rows, cols);

        let n = rows * cols;
        let mut vertices = Vec::with_capacity(n);

        // Create vertices
        for row in 0..rows {
            for col in 0..cols {
                let id = row * cols + col;
                let mut props = HashMap::new();
                props.insert("x".to_string(), serde_json::json!(col));
                props.insert("y".to_string(), serde_json::json!(row));
                props.insert("name".to_string(), serde_json::json!(format!("Node_{}_{}", row, col)));

                vertices.push(Vertex {
                    id,
                    label: "GridNode".to_string(),
                    properties: props,
                });
            }
        }

        // Create edges (4-connected grid)
        let mut edges = Vec::new();
        for row in 0..rows {
            for col in 0..cols {
                let id = row * cols + col;

                // Right neighbor
                if col < cols - 1 {
                    let mut props = HashMap::new();
                    props.insert("distance".to_string(), serde_json::json!(1.0));
                    edges.push(Edge {
                        label: "CONNECTED".to_string(),
                        start: id,
                        end: id + 1,
                        properties: props,
                    });
                }

                // Bottom neighbor
                if row < rows - 1 {
                    let mut props = HashMap::new();
                    props.insert("distance".to_string(), serde_json::json!(1.0));
                    edges.push(Edge {
                        label: "CONNECTED".to_string(),
                        start: id,
                        end: id + cols,
                        properties: props,
                    });
                }
            }
        }

        println!("Generated {} vertices and {} edges", vertices.len(), edges.len());
        GraphData { vertices, edges }
    }

    /// Generate balanced k-ary tree
    fn generate_tree(&mut self, depth: usize, branching: usize) -> GraphData {
        println!("Generating tree with depth {} and branching factor {}...", depth, branching);

        let mut vertices = Vec::new();
        let mut edges = Vec::new();
        let mut current_id = 0;

        // BFS to create tree
        let mut queue = vec![(0usize, 0usize)]; // (id, current_depth)

        while let Some((id, d)) = queue.pop() {
            // Create vertex
            let mut props = HashMap::new();
            props.insert("depth".to_string(), serde_json::json!(d));
            props.insert("name".to_string(), serde_json::json!(format!("Node{}", id)));

            vertices.push(Vertex {
                id,
                label: "TreeNode".to_string(),
                properties: props,
            });

            // Create children if not at max depth
            if d < depth {
                for i in 0..branching {
                    current_id += 1;
                    let child_id = current_id;

                    let mut props = HashMap::new();
                    props.insert("child_index".to_string(), serde_json::json!(i));

                    edges.push(Edge {
                        label: "PARENT_OF".to_string(),
                        start: id,
                        end: child_id,
                        properties: props,
                    });

                    queue.push((child_id, d + 1));
                }
            }
        }

        println!("Generated {} vertices and {} edges", vertices.len(), edges.len());
        GraphData { vertices, edges }
    }

    /// Export graph to JSON format
    fn export_json(&self, data: &GraphData, path: &Path) -> Result<()> {
        let file = File::create(path.join("graph.json"))?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, data)?;
        println!("Exported to JSON: {:?}", path.join("graph.json"));
        Ok(())
    }

    /// Export graph to CSV format (separate files for vertices and edges)
    fn export_csv(&self, data: &GraphData, path: &Path) -> Result<()> {
        // Export vertices
        let vertex_file = File::create(path.join("vertices.csv"))?;
        let mut vertex_writer = BufWriter::new(vertex_file);
        writeln!(vertex_writer, "id,label,properties")?;
        for v in &data.vertices {
            writeln!(
                vertex_writer,
                "{},{},\"{}\"",
                v.id,
                v.label,
                serde_json::to_string(&v.properties)?
            )?;
        }

        // Export edges
        let edge_file = File::create(path.join("edges.csv"))?;
        let mut edge_writer = BufWriter::new(edge_file);
        writeln!(edge_writer, "label,start,end,properties")?;
        for e in &data.edges {
            writeln!(
                edge_writer,
                "{},{},{},\"{}\"",
                e.label,
                e.start,
                e.end,
                serde_json::to_string(&e.properties)?
            )?;
        }

        println!("Exported to CSV: {:?}, {:?}",
                path.join("vertices.csv"),
                path.join("edges.csv"));
        Ok(())
    }

    /// Export graph to Cypher CREATE statements
    fn export_cypher(&self, data: &GraphData, path: &Path) -> Result<()> {
        let file = File::create(path.join("graph.cypher"))?;
        let mut writer = BufWriter::new(file);

        // Write vertex creation statements
        writeln!(writer, "-- Create Vertices")?;
        for v in &data.vertices {
            let props_str = v.properties.iter()
                .map(|(k, v)| {
                    match v {
                        serde_json::Value::String(s) => format!("{}: '{}'", k, s),
                        serde_json::Value::Number(n) => format!("{}: {}", k, n),
                        _ => format!("{}: '{}'", k, v.to_string()),
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");

            writeln!(
                writer,
                "CREATE (n{}:{} {{{}}});",
                v.id, v.label, props_str
            )?;
        }

        // Write edge creation statements
        writeln!(writer, "\n-- Create Edges")?;
        for e in &data.edges {
            let props_str = e.properties.iter()
                .map(|(k, v)| {
                    match v {
                        serde_json::Value::String(s) => format!("{}: '{}'", k, s),
                        serde_json::Value::Number(n) => format!("{}: {}", k, n),
                        _ => format!("{}: '{}'", k, v.to_string()),
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");

            writeln!(
                writer,
                "MATCH (a), (b) WHERE id(a) = {} AND id(b) = {} CREATE (a)-[:{} {{{}}}]->(b);",
                e.start, e.end, e.label, props_str
            )?;
        }

        println!("Exported to Cypher: {:?}", path.join("graph.cypher"));
        Ok(())
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Create output directory
    std::fs::create_dir_all(&args.output)
        .context("Failed to create output directory")?;

    // Initialize generator
    let mut generator = GraphGenerator::new(args.seed);

    // Generate graph based on type
    let graph_data = match args.graph_type {
        GraphType::Uniform => {
            generator.generate_uniform(args.vertices, args.avg_degree)
        }
        GraphType::PowerLaw => {
            generator.generate_power_law(args.vertices, args.avg_degree)
        }
        GraphType::Grid => {
            let size = args.size.unwrap_or(
                (args.vertices as f64).sqrt().ceil() as usize
            );
            generator.generate_grid(size, size)
        }
        GraphType::Tree => {
            generator.generate_tree(args.depth, args.branching)
        }
    };

    // Export to requested formats
    for format in &args.formats {
        match format.as_str() {
            "json" => generator.export_json(&graph_data, &args.output)?,
            "csv" => generator.export_csv(&graph_data, &args.output)?,
            "cypher" => generator.export_cypher(&graph_data, &args.output)?,
            _ => eprintln!("Unknown format: {}", format),
        }
    }

    println!("\n✅ Data generation complete!");
    println!("Output directory: {:?}", args.output);
    println!("Graph statistics:");
    println!("  - Vertices: {}", graph_data.vertices.len());
    println!("  - Edges: {}", graph_data.edges.len());
    if !graph_data.vertices.is_empty() {
        let avg_degree = (2 * graph_data.edges.len()) as f64 / graph_data.vertices.len() as f64;
        println!("  - Average degree: {:.2}", avg_degree);
    }

    Ok(())
}
