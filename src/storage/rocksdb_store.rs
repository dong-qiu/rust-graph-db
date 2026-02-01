/// RocksDB storage implementation
///
/// Key space design:
/// - Vertex:   v:{graph}:{label}:{locid} → JSONB
/// - Edge:     e:{graph}:{label}:{locid} → JSONB
/// - OutEdge:  o:{graph}:{src_vid}:{eid} → null
/// - InEdge:   i:{graph}:{dst_vid}:{eid} → null
/// - Label:    l:{graph}:{name} → labid
/// - Counter:  c:{graph}:{label} → max_locid

use super::error::{StorageError, StorageResult};
use super::transaction::RocksDbTransaction;
use super::{GraphStorage, GraphTransaction};
use crate::types::{Edge, Graphid, Vertex};
use async_trait::async_trait;
use rocksdb::{Options, DB};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// RocksDB-backed graph storage
pub struct RocksDbStorage {
    /// RocksDB database instance
    db: Arc<DB>,

    /// Graph name (namespace)
    graph_name: String,

    /// Label cache: name -> label ID
    label_cache: Arc<Mutex<HashMap<String, u16>>>,

    /// Reverse label cache: label ID -> name
    reverse_label_cache: Arc<Mutex<HashMap<u16, String>>>,

    /// Next label ID
    next_label_id: Arc<Mutex<u16>>,
}

impl RocksDbStorage {
    /// Create a new RocksDB storage instance
    ///
    /// # Arguments
    /// * `path` - Path to the database directory
    /// * `graph_name` - Name of the graph (namespace)
    pub fn new<P: AsRef<Path>>(path: P, graph_name: impl Into<String>) -> StorageResult<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        let db = DB::open(&opts, path)?;

        let storage = Self {
            db: Arc::new(db),
            graph_name: graph_name.into(),
            label_cache: Arc::new(Mutex::new(HashMap::new())),
            reverse_label_cache: Arc::new(Mutex::new(HashMap::new())),
            next_label_id: Arc::new(Mutex::new(1)),
        };

        // Load existing labels
        storage.load_labels()?;

        Ok(storage)
    }

    /// Load existing labels from database
    fn load_labels(&self) -> StorageResult<()> {
        let prefix = self.make_label_prefix();
        let iter = self.db.prefix_iterator(prefix.as_bytes());

        let mut max_label_id = 0u16;

        for item in iter {
            let (key, value) = item?;
            let key_str = std::str::from_utf8(&key)?;

            if let Some(label_name) = key_str.strip_prefix(&prefix) {
                let label_id = u16::from_le_bytes([value[0], value[1]]);

                self.label_cache
                    .lock()
                    .unwrap()
                    .insert(label_name.to_string(), label_id);

                self.reverse_label_cache
                    .lock()
                    .unwrap()
                    .insert(label_id, label_name.to_string());

                if label_id > max_label_id {
                    max_label_id = label_id;
                }
            }
        }

        *self.next_label_id.lock().unwrap() = max_label_id + 1;

        Ok(())
    }

    /// Get or create a label ID
    fn get_or_create_label(&self, label: &str) -> StorageResult<u16> {
        // Check cache first
        {
            let cache = self.label_cache.lock().unwrap();
            if let Some(&label_id) = cache.get(label) {
                return Ok(label_id);
            }
        }

        // Create new label
        let label_id = {
            let mut next_id = self.next_label_id.lock().unwrap();
            let id = *next_id;
            *next_id = next_id
                .checked_add(1)
                .ok_or_else(|| StorageError::CounterOverflow(label.to_string()))?;
            id
        };

        // Store in database
        let key = self.make_label_key(label);
        self.db.put(key.as_bytes(), &label_id.to_le_bytes())?;

        // Update caches
        self.label_cache
            .lock()
            .unwrap()
            .insert(label.to_string(), label_id);
        self.reverse_label_cache
            .lock()
            .unwrap()
            .insert(label_id, label.to_string());

        Ok(label_id)
    }

    /// Get label ID (without creating)
    fn get_label_id(&self, label: &str) -> StorageResult<u16> {
        // Check cache first
        if let Some(&label_id) = self.label_cache.lock().unwrap().get(label) {
            return Ok(label_id);
        }

        // Try to read from database
        let key = format!("l:{}:{}", self.graph_name, label);
        if let Some(bytes) = self.db.get(key.as_bytes())? {
            let label_id = u16::from_le_bytes([bytes[0], bytes[1]]);

            // Update cache
            self.label_cache
                .lock()
                .unwrap()
                .insert(label.to_string(), label_id);
            self.reverse_label_cache
                .lock()
                .unwrap()
                .insert(label_id, label.to_string());

            return Ok(label_id);
        }

        Err(StorageError::LabelNotFound(label.to_string()))
    }

    /// Get label name from ID
    fn get_label_name(&self, label_id: u16) -> StorageResult<String> {
        self.reverse_label_cache
            .lock()
            .unwrap()
            .get(&label_id)
            .cloned()
            .ok_or_else(|| StorageError::InvalidLabelId(label_id))
    }

    /// Get next local ID for a label
    fn next_local_id(&self, label: &str) -> StorageResult<u64> {
        let key = self.make_counter_key(label);

        let current = self
            .db
            .get(key.as_bytes())?
            .map(|bytes| {
                u64::from_le_bytes([
                    bytes[0], bytes[1], bytes[2], bytes[3],
                    bytes[4], bytes[5], bytes[6], bytes[7],
                ])
            })
            .unwrap_or(0);

        let next = current
            .checked_add(1)
            .ok_or_else(|| StorageError::CounterOverflow(label.to_string()))?;

        // Check if within 48-bit range
        if next > Graphid::MAX_LOCID {
            return Err(StorageError::CounterOverflow(label.to_string()));
        }

        self.db.put(key.as_bytes(), &next.to_le_bytes())?;

        Ok(next)
    }

    /// Make key prefix for labels
    fn make_label_prefix(&self) -> String {
        format!("l:{}:", self.graph_name)
    }

    /// Make key for a label
    fn make_label_key(&self, label: &str) -> String {
        format!("l:{}:{}", self.graph_name, label)
    }

    /// Make key for a counter
    fn make_counter_key(&self, label: &str) -> String {
        format!("c:{}:{}", self.graph_name, label)
    }

    /// Make key for a vertex
    fn make_vertex_key(&self, label_id: u16, locid: u64) -> String {
        format!("v:{}:{}:{}", self.graph_name, label_id, locid)
    }

    /// Make key prefix for vertex scan
    fn make_vertex_prefix(&self, label_id: u16) -> String {
        format!("v:{}:{}:", self.graph_name, label_id)
    }

    /// Make key for an edge
    fn make_edge_key(&self, label_id: u16, locid: u64) -> String {
        format!("e:{}:{}:{}", self.graph_name, label_id, locid)
    }

    /// Make key prefix for edge scan
    fn make_edge_prefix(&self, label_id: u16) -> String {
        format!("e:{}:{}:", self.graph_name, label_id)
    }

    /// Make key for outgoing edge index
    fn make_outgoing_key(&self, src: Graphid, eid: Graphid) -> String {
        format!("o:{}:{}:{}", self.graph_name, src.as_raw(), eid.as_raw())
    }

    /// Make key prefix for outgoing edges
    fn make_outgoing_prefix(&self, src: Graphid) -> String {
        format!("o:{}:{}:", self.graph_name, src.as_raw())
    }

    /// Make key for incoming edge index
    fn make_incoming_key(&self, dst: Graphid, eid: Graphid) -> String {
        format!("i:{}:{}:{}", self.graph_name, dst.as_raw(), eid.as_raw())
    }

    /// Make key prefix for incoming edges
    fn make_incoming_prefix(&self, dst: Graphid) -> String {
        format!("i:{}:{}:", self.graph_name, dst.as_raw())
    }

    /// Serialize vertex to JSON bytes
    fn serialize_vertex(&self, vertex: &Vertex) -> StorageResult<Vec<u8>> {
        Ok(serde_json::to_vec(vertex)?)
    }

    /// Deserialize vertex from JSON bytes
    fn deserialize_vertex(&self, bytes: &[u8]) -> StorageResult<Vertex> {
        Ok(serde_json::from_slice(bytes)?)
    }

    /// Serialize edge to JSON bytes
    fn serialize_edge(&self, edge: &Edge) -> StorageResult<Vec<u8>> {
        Ok(serde_json::to_vec(edge)?)
    }

    /// Deserialize edge from JSON bytes
    fn deserialize_edge(&self, bytes: &[u8]) -> StorageResult<Edge> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

#[async_trait]
impl GraphStorage for RocksDbStorage {
    async fn get_vertex(&self, id: Graphid) -> StorageResult<Option<Vertex>> {
        let key = self.make_vertex_key(id.labid(), id.locid());

        match self.db.get(key.as_bytes())? {
            Some(bytes) => {
                let vertex = self.deserialize_vertex(&bytes)?;
                Ok(Some(vertex))
            }
            None => Ok(None),
        }
    }

    async fn get_edge(&self, id: Graphid) -> StorageResult<Option<Edge>> {
        let key = self.make_edge_key(id.labid(), id.locid());

        match self.db.get(key.as_bytes())? {
            Some(bytes) => {
                let edge = self.deserialize_edge(&bytes)?;
                Ok(Some(edge))
            }
            None => Ok(None),
        }
    }

    async fn create_vertex(&self, label: &str, properties: JsonValue) -> StorageResult<Vertex> {
        let label_id = self.get_or_create_label(label)?;
        let locid = self.next_local_id(label)?;
        let id = Graphid::new(label_id, locid)
            .map_err(|e| StorageError::InvalidState(e.to_string()))?;

        let vertex = Vertex::new(id, label, properties);

        let key = self.make_vertex_key(label_id, locid);
        let value = self.serialize_vertex(&vertex)?;

        self.db.put(key.as_bytes(), &value)?;

        Ok(vertex)
    }

    async fn create_edge(
        &self,
        label: &str,
        start: Graphid,
        end: Graphid,
        properties: JsonValue,
    ) -> StorageResult<Edge> {
        let label_id = self.get_or_create_label(label)?;
        let locid = self.next_local_id(label)?;
        let id = Graphid::new(label_id, locid)
            .map_err(|e| StorageError::InvalidState(e.to_string()))?;

        let edge = Edge::new(id, start, end, label, properties);

        // Store edge
        let key = self.make_edge_key(label_id, locid);
        let value = self.serialize_edge(&edge)?;
        self.db.put(key.as_bytes(), &value)?;

        // Create indices
        let out_key = self.make_outgoing_key(start, id);
        self.db.put(out_key.as_bytes(), b"")?;

        let in_key = self.make_incoming_key(end, id);
        self.db.put(in_key.as_bytes(), b"")?;

        Ok(edge)
    }

    async fn delete_vertex(&self, id: Graphid) -> StorageResult<()> {
        // Check for connected edges
        let outgoing = self.get_outgoing_edges(id).await?;
        let incoming = self.get_incoming_edges(id).await?;
        let total_edges = outgoing.len() + incoming.len();

        if total_edges > 0 {
            return Err(StorageError::VertexHasEdges(id.to_string(), total_edges));
        }

        // Delete vertex
        let key = self.make_vertex_key(id.labid(), id.locid());
        self.db.delete(key.as_bytes())?;

        Ok(())
    }

    async fn delete_edge(&self, id: Graphid) -> StorageResult<()> {
        // Get edge to find start/end vertices
        let edge = self
            .get_edge(id)
            .await?
            .ok_or_else(|| StorageError::EdgeNotFound(id.to_string()))?;

        // Delete edge
        let key = self.make_edge_key(id.labid(), id.locid());
        self.db.delete(key.as_bytes())?;

        // Delete indices
        let out_key = self.make_outgoing_key(edge.start, id);
        self.db.delete(out_key.as_bytes())?;

        let in_key = self.make_incoming_key(edge.end, id);
        self.db.delete(in_key.as_bytes())?;

        Ok(())
    }

    async fn scan_vertices(&self, label: &str) -> StorageResult<Vec<Vertex>> {
        let label_id = self.get_label_id(label)?;
        let prefix = self.make_vertex_prefix(label_id);

        let mut vertices = Vec::new();
        let iter = self.db.prefix_iterator(prefix.as_bytes());

        for item in iter {
            let (key, value) = item?;
            let key_str = std::str::from_utf8(&key)?;

            // Check if key still matches prefix
            if !key_str.starts_with(&prefix) {
                break;
            }

            let vertex = self.deserialize_vertex(&value)?;
            vertices.push(vertex);
        }

        Ok(vertices)
    }

    async fn scan_edges(&self, label: &str) -> StorageResult<Vec<Edge>> {
        let label_id = self.get_label_id(label)?;
        let prefix = self.make_edge_prefix(label_id);

        let mut edges = Vec::new();
        let iter = self.db.prefix_iterator(prefix.as_bytes());

        for item in iter {
            let (key, value) = item?;
            let key_str = std::str::from_utf8(&key)?;

            // Check if key still matches prefix
            if !key_str.starts_with(&prefix) {
                break;
            }

            let edge = self.deserialize_edge(&value)?;
            edges.push(edge);
        }

        Ok(edges)
    }

    async fn get_outgoing_edges(&self, vid: Graphid) -> StorageResult<Vec<Edge>> {
        let prefix = self.make_outgoing_prefix(vid);
        let mut edges = Vec::new();

        let iter = self.db.prefix_iterator(prefix.as_bytes());

        for item in iter {
            let (key, _value) = item?;
            let key_str = std::str::from_utf8(&key)?;

            // Check if key still matches prefix
            if !key_str.starts_with(&prefix) {
                break;
            }

            // Extract edge ID from key: o:{graph}:{src}:{eid}
            if let Some(eid_str) = key_str.split(':').nth(3) {
                let eid_raw = eid_str
                    .parse::<u64>()
                    .map_err(|_| StorageError::InvalidState("Invalid edge ID in index".into()))?;
                let eid = Graphid::from_raw(eid_raw);

                if let Some(edge) = self.get_edge(eid).await? {
                    edges.push(edge);
                }
            }
        }

        Ok(edges)
    }

    async fn get_incoming_edges(&self, vid: Graphid) -> StorageResult<Vec<Edge>> {
        let prefix = self.make_incoming_prefix(vid);
        let mut edges = Vec::new();

        let iter = self.db.prefix_iterator(prefix.as_bytes());

        for item in iter {
            let (key, _value) = item?;
            let key_str = std::str::from_utf8(&key)?;

            // Check if key still matches prefix
            if !key_str.starts_with(&prefix) {
                break;
            }

            // Extract edge ID from key: i:{graph}:{dst}:{eid}
            if let Some(eid_str) = key_str.split(':').nth(3) {
                let eid_raw = eid_str
                    .parse::<u64>()
                    .map_err(|_| StorageError::InvalidState("Invalid edge ID in index".into()))?;
                let eid = Graphid::from_raw(eid_raw);

                if let Some(edge) = self.get_edge(eid).await? {
                    edges.push(edge);
                }
            }
        }

        Ok(edges)
    }

    async fn begin_transaction(&self) -> StorageResult<Box<dyn GraphTransaction>> {
        Ok(Box::new(RocksDbTransaction::new(self.db.clone(), self.graph_name.clone())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    async fn create_test_storage() -> (RocksDbStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage = RocksDbStorage::new(temp_dir.path(), "test_graph").unwrap();
        (storage, temp_dir)
    }

    #[tokio::test]
    async fn test_create_and_get_vertex() {
        let (storage, _temp) = create_test_storage().await;

        let vertex = storage
            .create_vertex("Person", json!({"name": "Alice", "age": 30}))
            .await
            .unwrap();

        assert_eq!(vertex.label, "Person");
        assert_eq!(vertex.get_property("name"), Some(&json!("Alice")));

        let retrieved = storage.get_vertex(vertex.id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, vertex.id);
    }

    #[tokio::test]
    async fn test_create_and_get_edge() {
        let (storage, _temp) = create_test_storage().await;

        let v1 = storage
            .create_vertex("Person", json!({"name": "Alice"}))
            .await
            .unwrap();
        let v2 = storage
            .create_vertex("Person", json!({"name": "Bob"}))
            .await
            .unwrap();

        let edge = storage
            .create_edge("KNOWS", v1.id, v2.id, json!({"since": 2020}))
            .await
            .unwrap();

        assert_eq!(edge.label, "KNOWS");
        assert_eq!(edge.start, v1.id);
        assert_eq!(edge.end, v2.id);

        let retrieved = storage.get_edge(edge.id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, edge.id);
    }

    #[tokio::test]
    async fn test_scan_vertices() {
        let (storage, _temp) = create_test_storage().await;

        storage
            .create_vertex("Person", json!({"name": "Alice"}))
            .await
            .unwrap();
        storage
            .create_vertex("Person", json!({"name": "Bob"}))
            .await
            .unwrap();
        storage
            .create_vertex("Company", json!({"name": "ACME"}))
            .await
            .unwrap();

        let people = storage.scan_vertices("Person").await.unwrap();
        assert_eq!(people.len(), 2);

        let companies = storage.scan_vertices("Company").await.unwrap();
        assert_eq!(companies.len(), 1);
    }

    #[tokio::test]
    async fn test_outgoing_incoming_edges() {
        let (storage, _temp) = create_test_storage().await;

        let v1 = storage
            .create_vertex("Person", json!({"name": "Alice"}))
            .await
            .unwrap();
        let v2 = storage
            .create_vertex("Person", json!({"name": "Bob"}))
            .await
            .unwrap();
        let v3 = storage
            .create_vertex("Person", json!({"name": "Carol"}))
            .await
            .unwrap();

        storage
            .create_edge("KNOWS", v1.id, v2.id, json!({}))
            .await
            .unwrap();
        storage
            .create_edge("KNOWS", v1.id, v3.id, json!({}))
            .await
            .unwrap();

        let outgoing = storage.get_outgoing_edges(v1.id).await.unwrap();
        assert_eq!(outgoing.len(), 2);

        let incoming = storage.get_incoming_edges(v2.id).await.unwrap();
        assert_eq!(incoming.len(), 1);
    }

    #[tokio::test]
    async fn test_delete_vertex_with_edges_fails() {
        let (storage, _temp) = create_test_storage().await;

        let v1 = storage
            .create_vertex("Person", json!({"name": "Alice"}))
            .await
            .unwrap();
        let v2 = storage
            .create_vertex("Person", json!({"name": "Bob"}))
            .await
            .unwrap();

        storage
            .create_edge("KNOWS", v1.id, v2.id, json!({}))
            .await
            .unwrap();

        let result = storage.delete_vertex(v1.id).await;
        assert!(result.is_err());
        assert!(matches!(result, Err(StorageError::VertexHasEdges(_, _))));
    }

    #[tokio::test]
    async fn test_delete_edge() {
        let (storage, _temp) = create_test_storage().await;

        let v1 = storage
            .create_vertex("Person", json!({"name": "Alice"}))
            .await
            .unwrap();
        let v2 = storage
            .create_vertex("Person", json!({"name": "Bob"}))
            .await
            .unwrap();

        let edge = storage
            .create_edge("KNOWS", v1.id, v2.id, json!({}))
            .await
            .unwrap();

        storage.delete_edge(edge.id).await.unwrap();

        let retrieved = storage.get_edge(edge.id).await.unwrap();
        assert!(retrieved.is_none());

        let outgoing = storage.get_outgoing_edges(v1.id).await.unwrap();
        assert_eq!(outgoing.len(), 0);
    }
}
