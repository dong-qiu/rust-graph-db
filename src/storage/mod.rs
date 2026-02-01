/// Storage engine abstraction for the graph database
///
/// This module provides the core storage interface and implementations:
/// - GraphStorage trait: Abstract storage operations
/// - RocksDB implementation: Production storage backend
/// - Transaction support: ACID guarantees

pub mod error;
pub mod rocksdb_store;
pub mod transaction;

use crate::types::{Edge, Graphid, Vertex};
use async_trait::async_trait;
pub use error::{StorageError, StorageResult};
use serde_json::Value as JsonValue;
use std::sync::Arc;

/// Graph storage abstraction
///
/// This trait defines the core operations for storing and retrieving graph data.
/// Implementations must provide:
/// - Vertex operations (get, create, delete, scan)
/// - Edge operations (get, create, delete, scan)
/// - Relationship queries (outgoing/incoming edges)
/// - Transaction support
#[async_trait]
pub trait GraphStorage: Send + Sync {
    /// Get a vertex by ID
    ///
    /// # Arguments
    /// * `id` - The vertex Graphid
    ///
    /// # Returns
    /// * `Ok(Some(vertex))` if found
    /// * `Ok(None)` if not found
    /// * `Err(StorageError)` on storage errors
    async fn get_vertex(&self, id: Graphid) -> StorageResult<Option<Vertex>>;

    /// Get an edge by ID
    ///
    /// # Arguments
    /// * `id` - The edge Graphid
    ///
    /// # Returns
    /// * `Ok(Some(edge))` if found
    /// * `Ok(None)` if not found
    /// * `Err(StorageError)` on storage errors
    async fn get_edge(&self, id: Graphid) -> StorageResult<Option<Edge>>;

    /// Create a new vertex
    ///
    /// # Arguments
    /// * `label` - The vertex label (type)
    /// * `properties` - JSON properties
    ///
    /// # Returns
    /// * `Ok(vertex)` with assigned ID
    /// * `Err(StorageError)` on storage errors
    async fn create_vertex(&self, label: &str, properties: JsonValue) -> StorageResult<Vertex>;

    /// Create a new edge
    ///
    /// # Arguments
    /// * `label` - The edge label (relationship type)
    /// * `start` - Start vertex ID
    /// * `end` - End vertex ID
    /// * `properties` - JSON properties
    ///
    /// # Returns
    /// * `Ok(edge)` with assigned ID
    /// * `Err(StorageError)` on storage errors
    async fn create_edge(
        &self,
        label: &str,
        start: Graphid,
        end: Graphid,
        properties: JsonValue,
    ) -> StorageResult<Edge>;

    /// Delete a vertex
    ///
    /// # Arguments
    /// * `id` - The vertex ID to delete
    ///
    /// # Returns
    /// * `Ok(())` if deleted successfully
    /// * `Err(StorageError)` on errors (including if vertex has edges)
    async fn delete_vertex(&self, id: Graphid) -> StorageResult<()>;

    /// Delete an edge
    ///
    /// # Arguments
    /// * `id` - The edge ID to delete
    ///
    /// # Returns
    /// * `Ok(())` if deleted successfully
    /// * `Err(StorageError)` on errors
    async fn delete_edge(&self, id: Graphid) -> StorageResult<()>;

    /// Scan all vertices with a given label
    ///
    /// # Arguments
    /// * `label` - The vertex label to filter by
    ///
    /// # Returns
    /// * `Ok(Vec<Vertex>)` - All matching vertices
    /// * `Err(StorageError)` on storage errors
    async fn scan_vertices(&self, label: &str) -> StorageResult<Vec<Vertex>>;

    /// Scan all edges with a given label
    ///
    /// # Arguments
    /// * `label` - The edge label to filter by
    ///
    /// # Returns
    /// * `Ok(Vec<Edge>)` - All matching edges
    /// * `Err(StorageError)` on storage errors
    async fn scan_edges(&self, label: &str) -> StorageResult<Vec<Edge>>;

    /// Get all outgoing edges from a vertex
    ///
    /// # Arguments
    /// * `vid` - The source vertex ID
    ///
    /// # Returns
    /// * `Ok(Vec<Edge>)` - All outgoing edges
    /// * `Err(StorageError)` on storage errors
    async fn get_outgoing_edges(&self, vid: Graphid) -> StorageResult<Vec<Edge>>;

    /// Get all incoming edges to a vertex
    ///
    /// # Arguments
    /// * `vid` - The target vertex ID
    ///
    /// # Returns
    /// * `Ok(Vec<Edge>)` - All incoming edges
    /// * `Err(StorageError)` on storage errors
    async fn get_incoming_edges(&self, vid: Graphid) -> StorageResult<Vec<Edge>>;

    /// Begin a transaction
    ///
    /// # Returns
    /// * `Ok(transaction)` - A new transaction handle
    /// * `Err(StorageError)` on errors
    async fn begin_transaction(&self) -> StorageResult<Box<dyn GraphTransaction>>;
}

/// Transaction interface for graph operations
///
/// Provides ACID guarantees for graph modifications
#[async_trait]
pub trait GraphTransaction: Send + Sync {
    /// Get a vertex by ID within this transaction
    async fn get_vertex(&self, id: Graphid) -> StorageResult<Option<Vertex>>;

    /// Get an edge by ID within this transaction
    async fn get_edge(&self, id: Graphid) -> StorageResult<Option<Edge>>;

    /// Get all outgoing edges from a vertex
    async fn get_outgoing_edges(&self, vid: Graphid) -> StorageResult<Vec<Edge>>;

    /// Get all incoming edges to a vertex
    async fn get_incoming_edges(&self, vid: Graphid) -> StorageResult<Vec<Edge>>;

    /// Create a vertex within this transaction
    async fn create_vertex(&mut self, label: &str, properties: JsonValue)
        -> StorageResult<Vertex>;

    /// Create an edge within this transaction
    async fn create_edge(
        &mut self,
        label: &str,
        start: Graphid,
        end: Graphid,
        properties: JsonValue,
    ) -> StorageResult<Edge>;

    /// Update a vertex's properties
    async fn update_vertex(&mut self, id: Graphid, properties: JsonValue) -> StorageResult<()>;

    /// Update an edge's properties
    async fn update_edge(&mut self, id: Graphid, properties: JsonValue) -> StorageResult<()>;

    /// Delete a vertex within this transaction
    async fn delete_vertex(&mut self, id: Graphid) -> StorageResult<()>;

    /// Delete an edge within this transaction
    async fn delete_edge(&mut self, id: Graphid) -> StorageResult<()>;

    /// Commit the transaction
    async fn commit(&mut self) -> StorageResult<()>;

    /// Rollback the transaction
    async fn rollback(&mut self) -> StorageResult<()>;
}

/// Shared storage handle
pub type SharedStorage = Arc<dyn GraphStorage>;
