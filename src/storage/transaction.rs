/// Transaction implementation for RocksDB storage
///
/// Provides ACID guarantees for graph operations using RocksDB's WriteBatch

use super::error::{StorageError, StorageResult};
use super::GraphTransaction;
use crate::types::{Edge, Graphid, Vertex};
use async_trait::async_trait;
use rocksdb::{WriteBatch, DB};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;

/// Write operation for batching
#[derive(Debug, Clone)]
enum WriteOp {
    Put { key: Vec<u8>, value: Vec<u8> },
    Delete { key: Vec<u8> },
}

/// RocksDB transaction using WriteBatch
///
/// This implementation provides atomicity by batching all writes
/// and committing them together.
pub struct RocksDbTransaction {
    /// Reference to the database
    db: Arc<DB>,

    /// Graph name (namespace)
    graph_name: String,

    /// Batched write operations
    operations: Vec<WriteOp>,

    /// Pending vertex creations
    pending_vertices: Vec<Vertex>,

    /// Pending edge creations
    pending_edges: Vec<Edge>,

    /// Label cache for this transaction
    label_cache: HashMap<String, u16>,

    /// Next label ID
    next_label_id: u16,

    /// Counter cache for local IDs
    counter_cache: HashMap<String, u64>,

    /// Transaction state
    committed: bool,
    rolled_back: bool,
}

impl RocksDbTransaction {
    /// Create a new transaction
    pub fn new(db: Arc<DB>, graph_name: String) -> Self {
        Self {
            db,
            graph_name,
            operations: Vec::new(),
            pending_vertices: Vec::new(),
            pending_edges: Vec::new(),
            label_cache: HashMap::new(),
            next_label_id: 1,
            counter_cache: HashMap::new(),
            committed: false,
            rolled_back: false,
        }
    }

    /// Check transaction state
    fn check_state(&self) -> StorageResult<()> {
        if self.committed {
            return Err(StorageError::TransactionError(
                "Transaction already committed".into(),
            ));
        }
        if self.rolled_back {
            return Err(StorageError::TransactionError(
                "Transaction already rolled back".into(),
            ));
        }
        Ok(())
    }

    /// Add a put operation to the batch
    fn put(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.operations.push(WriteOp::Put { key, value });
    }

    /// Add a delete operation to the batch
    fn delete(&mut self, key: Vec<u8>) {
        self.operations.push(WriteOp::Delete { key });
    }

    /// Get or create label ID
    fn get_or_create_label(&mut self, label: &str) -> StorageResult<u16> {
        // Check cache
        if let Some(&label_id) = self.label_cache.get(label) {
            return Ok(label_id);
        }

        // Check database
        let key = format!("l:{}:{}", self.graph_name, label);
        if let Some(bytes) = self.db.get(key.as_bytes())? {
            let label_id = u16::from_le_bytes([bytes[0], bytes[1]]);
            self.label_cache.insert(label.to_string(), label_id);
            return Ok(label_id);
        }

        // Create new label
        let label_id = self.next_label_id;
        self.next_label_id = label_id
            .checked_add(1)
            .ok_or_else(|| StorageError::CounterOverflow(label.to_string()))?;

        // Add to batch
        self.put(key.into_bytes(), label_id.to_le_bytes().to_vec());
        self.label_cache.insert(label.to_string(), label_id);

        Ok(label_id)
    }

    /// Get next local ID for a label
    fn next_local_id(&mut self, label: &str) -> StorageResult<u64> {
        // Check cache
        if let Some(&current) = self.counter_cache.get(label) {
            let next = current
                .checked_add(1)
                .ok_or_else(|| StorageError::CounterOverflow(label.to_string()))?;

            if next > Graphid::MAX_LOCID {
                return Err(StorageError::CounterOverflow(label.to_string()));
            }

            self.counter_cache.insert(label.to_string(), next);
            return Ok(next);
        }

        // Read from database
        let key = format!("c:{}:{}", self.graph_name, label);
        let current = self
            .db
            .get(key.as_bytes())?
            .map(|bytes| u64::from_le_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3],
                bytes[4], bytes[5], bytes[6], bytes[7],
            ]))
            .unwrap_or(0);

        let next = current
            .checked_add(1)
            .ok_or_else(|| StorageError::CounterOverflow(label.to_string()))?;

        if next > Graphid::MAX_LOCID {
            return Err(StorageError::CounterOverflow(label.to_string()));
        }

        self.counter_cache.insert(label.to_string(), next);
        Ok(next)
    }

    /// Make key for a vertex
    fn make_vertex_key(&self, label_id: u16, locid: u64) -> Vec<u8> {
        format!("v:{}:{}:{}", self.graph_name, label_id, locid).into_bytes()
    }

    /// Make key for an edge
    fn make_edge_key(&self, label_id: u16, locid: u64) -> Vec<u8> {
        format!("e:{}:{}:{}", self.graph_name, label_id, locid).into_bytes()
    }

    /// Make key for outgoing edge index
    fn make_outgoing_key(&self, src: Graphid, eid: Graphid) -> Vec<u8> {
        format!("o:{}:{}:{}", self.graph_name, src.as_raw(), eid.as_raw()).into_bytes()
    }

    /// Make key for incoming edge index
    fn make_incoming_key(&self, dst: Graphid, eid: Graphid) -> Vec<u8> {
        format!("i:{}:{}:{}", self.graph_name, dst.as_raw(), eid.as_raw()).into_bytes()
    }
}

#[async_trait]
impl GraphTransaction for RocksDbTransaction {
    async fn get_vertex(&self, id: Graphid) -> StorageResult<Option<Vertex>> {
        let key = self.make_vertex_key(id.labid(), id.locid());

        if let Some(bytes) = self.db.get(&key)? {
            let vertex: Vertex = serde_json::from_slice(&bytes)?;
            Ok(Some(vertex))
        } else {
            Ok(None)
        }
    }

    async fn get_edge(&self, id: Graphid) -> StorageResult<Option<Edge>> {
        let key = self.make_edge_key(id.labid(), id.locid());

        if let Some(bytes) = self.db.get(&key)? {
            let edge: Edge = serde_json::from_slice(&bytes)?;
            Ok(Some(edge))
        } else {
            Ok(None)
        }
    }

    async fn get_outgoing_edges(&self, vid: Graphid) -> StorageResult<Vec<Edge>> {
        let prefix = format!("o:{}:{}:", self.graph_name, vid.as_raw());
        let iter = self.db.prefix_iterator(prefix.as_bytes());

        let mut edges = Vec::new();

        for item in iter {
            let (key, _) = item?;
            let key_str = String::from_utf8_lossy(&key);

            if !key_str.starts_with(&prefix) {
                break;
            }

            // Extract edge ID from key
            let parts: Vec<&str> = key_str.split(':').collect();
            if parts.len() >= 4 {
                if let Ok(eid_raw) = parts[3].parse::<u64>() {
                    let eid = Graphid::from_raw(eid_raw);
                    if let Some(edge) = self.get_edge(eid).await? {
                        edges.push(edge);
                    }
                }
            }
        }

        Ok(edges)
    }

    async fn get_incoming_edges(&self, vid: Graphid) -> StorageResult<Vec<Edge>> {
        let prefix = format!("i:{}:{}:", self.graph_name, vid.as_raw());
        let iter = self.db.prefix_iterator(prefix.as_bytes());

        let mut edges = Vec::new();

        for item in iter {
            let (key, _) = item?;
            let key_str = String::from_utf8_lossy(&key);

            if !key_str.starts_with(&prefix) {
                break;
            }

            // Extract edge ID from key
            let parts: Vec<&str> = key_str.split(':').collect();
            if parts.len() >= 4 {
                if let Ok(eid_raw) = parts[3].parse::<u64>() {
                    let eid = Graphid::from_raw(eid_raw);
                    if let Some(edge) = self.get_edge(eid).await? {
                        edges.push(edge);
                    }
                }
            }
        }

        Ok(edges)
    }

    async fn create_vertex(
        &mut self,
        label: &str,
        properties: JsonValue,
    ) -> StorageResult<Vertex> {
        self.check_state()?;

        let label_id = self.get_or_create_label(label)?;
        let locid = self.next_local_id(label)?;
        let id =
            Graphid::new(label_id, locid).map_err(|e| StorageError::InvalidState(e.to_string()))?;

        let vertex = Vertex::new(id, label, properties);

        // Add to batch
        let key = self.make_vertex_key(label_id, locid);
        let value = serde_json::to_vec(&vertex)?;
        self.put(key, value);

        // Track pending vertex
        self.pending_vertices.push(vertex.clone());

        Ok(vertex)
    }

    async fn create_edge(
        &mut self,
        label: &str,
        start: Graphid,
        end: Graphid,
        properties: JsonValue,
    ) -> StorageResult<Edge> {
        self.check_state()?;

        let label_id = self.get_or_create_label(label)?;
        let locid = self.next_local_id(label)?;
        let id =
            Graphid::new(label_id, locid).map_err(|e| StorageError::InvalidState(e.to_string()))?;

        let edge = Edge::new(id, start, end, label, properties);

        // Add edge to batch
        let key = self.make_edge_key(label_id, locid);
        let value = serde_json::to_vec(&edge)?;
        self.put(key, value);

        // Add indices to batch
        let out_key = self.make_outgoing_key(start, id);
        self.put(out_key, Vec::new());

        let in_key = self.make_incoming_key(end, id);
        self.put(in_key, Vec::new());

        // Track pending edge
        self.pending_edges.push(edge.clone());

        Ok(edge)
    }

    async fn update_vertex(&mut self, id: Graphid, properties: JsonValue) -> StorageResult<()> {
        self.check_state()?;

        // Get existing vertex to preserve label
        let vertex = self
            .get_vertex(id)
            .await?
            .ok_or_else(|| StorageError::VertexNotFound(format!("{:?}", id)))?;

        // Create updated vertex
        let updated_vertex = Vertex::new(id, &vertex.label, properties);

        // Update in batch
        let key = self.make_vertex_key(id.labid(), id.locid());
        let value = serde_json::to_vec(&updated_vertex)?;
        self.put(key, value);

        Ok(())
    }

    async fn update_edge(&mut self, id: Graphid, properties: JsonValue) -> StorageResult<()> {
        self.check_state()?;

        // Get existing edge to preserve label and vertices
        let edge = self
            .get_edge(id)
            .await?
            .ok_or_else(|| StorageError::EdgeNotFound(format!("{:?}", id)))?;

        // Create updated edge
        let updated_edge = Edge::new(id, edge.start, edge.end, &edge.label, properties);

        // Update in batch
        let key = self.make_edge_key(id.labid(), id.locid());
        let value = serde_json::to_vec(&updated_edge)?;
        self.put(key, value);

        Ok(())
    }

    async fn delete_vertex(&mut self, id: Graphid) -> StorageResult<()> {
        self.check_state()?;

        let key = self.make_vertex_key(id.labid(), id.locid());
        self.delete(key);

        Ok(())
    }

    async fn delete_edge(&mut self, id: Graphid) -> StorageResult<()> {
        self.check_state()?;

        // Get edge to find start/end vertices
        let edge_key_str = format!("e:{}:{}:{}", self.graph_name, id.labid(), id.locid());

        // Read edge from database
        if let Some(bytes) = self.db.get(edge_key_str.as_bytes())? {
            let edge: Edge = serde_json::from_slice(&bytes)?;

            // Delete edge
            self.delete(edge_key_str.into_bytes());

            // Delete indices
            let out_key = self.make_outgoing_key(edge.start, id);
            self.delete(out_key);

            let in_key = self.make_incoming_key(edge.end, id);
            self.delete(in_key);
        }

        Ok(())
    }

    async fn commit(&mut self) -> StorageResult<()> {
        self.check_state()?;

        // Create WriteBatch
        let mut batch = WriteBatch::default();

        // Add counter updates
        for (label, &counter) in &self.counter_cache {
            let key = format!("c:{}:{}", self.graph_name, label);
            batch.put(key.as_bytes(), &counter.to_le_bytes());
        }

        // Add all operations to batch
        for op in &self.operations {
            match op {
                WriteOp::Put { key, value } => {
                    batch.put(key, value);
                }
                WriteOp::Delete { key } => {
                    batch.delete(key);
                }
            }
        }

        // Commit the batch atomically
        self.db.write(batch)?;

        self.committed = true;
        Ok(())
    }

    async fn rollback(&mut self) -> StorageResult<()> {
        self.check_state()?;

        // Simply clear operations and mark as rolled back
        self.operations.clear();
        self.rolled_back = true;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::rocksdb_store::RocksDbStorage;
    use crate::storage::GraphStorage;
    use serde_json::json;
    use tempfile::TempDir;

    async fn create_test_storage() -> (RocksDbStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage = RocksDbStorage::new(temp_dir.path(), "test_graph").unwrap();
        (storage, temp_dir)
    }

    #[tokio::test]
    async fn test_transaction_commit() {
        let (storage, _temp) = create_test_storage().await;

        let mut tx = storage.begin_transaction().await.unwrap();

        let v1 = tx
            .create_vertex("Person", json!({"name": "Alice"}))
            .await
            .unwrap();
        let v2 = tx
            .create_vertex("Person", json!({"name": "Bob"}))
            .await
            .unwrap();

        tx.create_edge("KNOWS", v1.id, v2.id, json!({"since": 2020}))
            .await
            .unwrap();

        tx.commit().await.unwrap();

        // Verify data is persisted
        let retrieved_v1 = storage.get_vertex(v1.id).await.unwrap();
        assert!(retrieved_v1.is_some());
        assert_eq!(
            retrieved_v1.unwrap().get_property("name"),
            Some(&json!("Alice"))
        );
    }

    #[tokio::test]
    async fn test_transaction_rollback() {
        let (storage, _temp) = create_test_storage().await;

        let mut tx = storage.begin_transaction().await.unwrap();

        let v1 = tx
            .create_vertex("Person", json!({"name": "Alice"}))
            .await
            .unwrap();

        tx.rollback().await.unwrap();

        // Verify data is not persisted
        let retrieved = storage.get_vertex(v1.id).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_transaction_cannot_use_after_commit() {
        let (storage, _temp) = create_test_storage().await;

        let mut tx = storage.begin_transaction().await.unwrap();

        tx.commit().await.unwrap();

        // Should fail
        let result = tx.create_vertex("Person", json!({"name": "Bob"})).await;
        assert!(result.is_err());
    }
}
