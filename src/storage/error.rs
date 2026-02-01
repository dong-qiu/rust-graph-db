/// Error types for storage operations

use thiserror::Error;

/// Storage operation errors
#[derive(Error, Debug)]
pub enum StorageError {
    /// Vertex not found
    #[error("Vertex not found: {0}")]
    VertexNotFound(String),

    /// Edge not found
    #[error("Edge not found: {0}")]
    EdgeNotFound(String),

    /// Vertex has edges and cannot be deleted
    #[error("Cannot delete vertex {0}: has {1} connected edges")]
    VertexHasEdges(String, usize),

    /// Label not found
    #[error("Label not found: {0}")]
    LabelNotFound(String),

    /// Invalid label ID
    #[error("Invalid label ID: {0}")]
    InvalidLabelId(u16),

    /// Counter overflow
    #[error("Counter overflow for label: {0}")]
    CounterOverflow(String),

    /// Database error
    #[error("Database error: {0}")]
    DatabaseError(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// RocksDB error
    #[error("RocksDB error: {0}")]
    RocksDbError(#[from] rocksdb::Error),

    /// Transaction error
    #[error("Transaction error: {0}")]
    TransactionError(String),

    /// Concurrent modification detected
    #[error("Concurrent modification detected for key: {0}")]
    ConcurrentModification(String),

    /// Invalid state
    #[error("Invalid state: {0}")]
    InvalidState(String),

    /// UTF-8 conversion error
    #[error("UTF-8 conversion error: {0}")]
    Utf8Error(#[from] std::str::Utf8Error),

    /// Generic error
    #[error("Storage error: {0}")]
    Other(String),
}

/// Result type for storage operations
pub type StorageResult<T> = Result<T, StorageError>;

impl From<String> for StorageError {
    fn from(s: String) -> Self {
        StorageError::Other(s)
    }
}

impl From<&str> for StorageError {
    fn from(s: &str) -> Self {
        StorageError::Other(s.to_string())
    }
}
