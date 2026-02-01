/// Graph algorithms module
///
/// This module implements common graph algorithms for Cypher queries.

pub mod shortest_path;
pub mod vle;

pub use shortest_path::{dijkstra, shortest_path, ShortestPathResult};
pub use vle::{variable_length_expand, VariableLengthPath, VleOptions};

use crate::types::Graphid;
use thiserror::Error;

/// Algorithm errors
#[derive(Error, Debug)]
pub enum AlgorithmError {
    #[error("Storage error: {0}")]
    StorageError(#[from] crate::storage::StorageError),

    #[error("Path not found between {0:?} and {1:?}")]
    PathNotFound(Graphid, Graphid),

    #[error("Invalid parameters: {0}")]
    InvalidParameters(String),

    #[error("Graph algorithm error: {0}")]
    AlgorithmFailed(String),
}

pub type AlgorithmResult<T> = Result<T, AlgorithmError>;
