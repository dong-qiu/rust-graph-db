/// Core data types for the graph database
///
/// This module defines the fundamental types used throughout the system:
/// - Graphid: 64-bit identifier with embedded label and local ID
/// - Vertex: Graph node with properties
/// - Edge: Graph relationship with properties
/// - GraphPath: Sequence of vertices and edges forming a path

pub mod edge;
pub mod graphid;
pub mod path;
pub mod vertex;

pub use edge::Edge;
pub use graphid::{Graphid, GraphidError};
pub use path::{GraphPath, PathError};
pub use vertex::Vertex;
