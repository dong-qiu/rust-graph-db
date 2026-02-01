/// JSONB compatibility layer for openGauss-graph
///
/// This module provides functions to encode/decode JSON data in a format
/// compatible with PostgreSQL/openGauss JSONB binary format.
///
/// JSONB Format Overview:
/// - 4-byte header: version + flags + count
/// - Array of JEntry structures (4 bytes each)
/// - Data area containing actual values
///
/// For MVP, we use standard JSON serialization with serde_json.
/// Full binary JSONB compatibility will be implemented in phase 2.

use serde_json::Value as JsonValue;
use std::io::{Cursor, Read, Write};
use thiserror::Error;

/// Error types for JSONB operations
#[derive(Error, Debug)]
pub enum JsonbError {
    #[error("Invalid JSONB header: {0}")]
    InvalidHeader(String),

    #[error("Unsupported JSONB version: {0}")]
    UnsupportedVersion(u8),

    #[error("JSON serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Invalid data: {0}")]
    InvalidData(String),
}

/// JSONB container compatible with PostgreSQL format
///
/// For MVP: stores JSON as UTF-8 string internally
/// TODO Phase 2: Implement full binary JSONB format
#[derive(Debug, Clone, PartialEq)]
pub struct JsonbContainer {
    /// Internal JSON value
    value: JsonValue,
}

impl JsonbContainer {
    /// Create a new JSONB container from a JSON value
    pub fn new(value: JsonValue) -> Self {
        Self { value }
    }

    /// Parse JSONB from PostgreSQL binary format
    ///
    /// Current implementation: simplified format
    /// TODO Phase 2: Full PostgreSQL JSONB binary format
    pub fn from_postgres_bytes(bytes: &[u8]) -> Result<Self, JsonbError> {
        // For MVP: assume bytes are UTF-8 encoded JSON
        // This allows basic compatibility while we develop full JSONB support
        let json_str = std::str::from_utf8(bytes)
            .map_err(|e| JsonbError::InvalidData(format!("Invalid UTF-8: {}", e)))?;

        let value: JsonValue = serde_json::from_str(json_str)?;
        Ok(Self::new(value))
    }

    /// Convert to JSON value
    pub fn to_json_value(&self) -> &JsonValue {
        &self.value
    }

    /// Take ownership of the JSON value
    pub fn into_json_value(self) -> JsonValue {
        self.value
    }

    /// Encode to PostgreSQL JSONB binary format
    ///
    /// Current implementation: simplified format
    /// TODO Phase 2: Full PostgreSQL JSONB binary format
    pub fn to_postgres_bytes(&self) -> Result<Vec<u8>, JsonbError> {
        // For MVP: encode as UTF-8 JSON string
        let json_str = serde_json::to_string(&self.value)?;
        Ok(json_str.into_bytes())
    }

    /// Create JSONB from JSON value
    pub fn from_json_value(value: JsonValue) -> Self {
        Self::new(value)
    }
}

/// Full binary JSONB format implementation (PostgreSQL compatible)
///
/// This module will be fully implemented in Phase 2
#[allow(dead_code)]
mod binary_format {
    use super::*;

    // JSONB header flags
    const JB_FSCALAR: u32 = 0x10000000;
    const JB_FOBJECT: u32 = 0x20000000;
    const JB_FARRAY: u32 = 0x40000000;
    const JB_CMASK: u32 = 0x0FFFFFFF;

    // JEntry type flags
    const JENTRY_ISSTRING: u32 = 0x00000000;
    const JENTRY_ISNUMERIC: u32 = 0x10000000;
    const JENTRY_ISNEST: u32 = 0x20000000;
    const JENTRY_ISNULL: u32 = 0x40000000;
    const JENTRY_ISBOOL: u32 = JENTRY_ISNUMERIC | JENTRY_ISNEST;
    const JENTRY_ISTRUE: u32 = JENTRY_ISBOOL | 0x40000000;

    #[derive(Debug)]
    struct JsonbHeader {
        count: u32,
        flags: u32,
    }

    impl JsonbHeader {
        fn new(count: u32, flags: u32) -> Self {
            Self { count, flags }
        }

        fn encode(&self) -> u32 {
            (self.flags & !JB_CMASK) | (self.count & JB_CMASK)
        }

        fn decode(value: u32) -> Self {
            Self {
                count: value & JB_CMASK,
                flags: value & !JB_CMASK,
            }
        }

        fn is_scalar(&self) -> bool {
            (self.flags & JB_FSCALAR) != 0
        }

        fn is_object(&self) -> bool {
            (self.flags & JB_FOBJECT) != 0
        }

        fn is_array(&self) -> bool {
            (self.flags & JB_FARRAY) != 0
        }
    }

    #[derive(Debug)]
    struct JEntry {
        header: u32,
    }

    impl JEntry {
        fn new_string(offset: u32, length: u32) -> Self {
            Self {
                header: JENTRY_ISSTRING | (offset + length),
            }
        }

        fn new_null() -> Self {
            Self {
                header: JENTRY_ISNULL,
            }
        }

        fn new_bool(value: bool) -> Self {
            Self {
                header: if value {
                    JENTRY_ISTRUE
                } else {
                    JENTRY_ISBOOL
                },
            }
        }

        fn get_type(&self) -> u32 {
            self.header & 0x70000000
        }

        fn get_end_pos(&self) -> u32 {
            self.header & 0x0FFFFFFF
        }
    }

    /// TODO Phase 2: Implement full JSONB encoder
    #[allow(dead_code)]
    pub fn encode_jsonb(value: &JsonValue) -> Result<Vec<u8>, JsonbError> {
        let mut buf = Vec::new();

        match value {
            JsonValue::Object(map) => {
                let count = map.len() as u32;
                let header = JsonbHeader::new(count, JB_FOBJECT);
                buf.write_all(&header.encode().to_le_bytes())?;

                // TODO: Encode JEntry array and data
            }
            JsonValue::Array(arr) => {
                let count = arr.len() as u32;
                let header = JsonbHeader::new(count, JB_FARRAY);
                buf.write_all(&header.encode().to_le_bytes())?;

                // TODO: Encode JEntry array and data
            }
            _ => {
                let header = JsonbHeader::new(1, JB_FSCALAR);
                buf.write_all(&header.encode().to_le_bytes())?;

                // TODO: Encode scalar value
            }
        }

        Ok(buf)
    }

    /// TODO Phase 2: Implement full JSONB decoder
    #[allow(dead_code)]
    pub fn decode_jsonb(bytes: &[u8]) -> Result<JsonValue, JsonbError> {
        if bytes.len() < 4 {
            return Err(JsonbError::InvalidData("JSONB too short".to_string()));
        }

        let mut cursor = Cursor::new(bytes);
        let mut header_bytes = [0u8; 4];
        cursor.read_exact(&mut header_bytes)?;

        let header_value = u32::from_le_bytes(header_bytes);
        let header = JsonbHeader::decode(header_value);

        // TODO: Decode based on type
        if header.is_object() {
            // Decode object
            Ok(JsonValue::Object(serde_json::Map::new()))
        } else if header.is_array() {
            // Decode array
            Ok(JsonValue::Array(Vec::new()))
        } else if header.is_scalar() {
            // Decode scalar
            Ok(JsonValue::Null)
        } else {
            Err(JsonbError::InvalidHeader(format!(
                "Unknown JSONB type: {:x}",
                header.flags
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_jsonb_container_creation() {
        let value = json!({"name": "Alice", "age": 30});
        let container = JsonbContainer::new(value.clone());

        assert_eq!(container.to_json_value(), &value);
    }

    #[test]
    fn test_jsonb_from_json_value() {
        let value = json!({"name": "Bob"});
        let container = JsonbContainer::from_json_value(value.clone());

        assert_eq!(container.into_json_value(), value);
    }

    #[test]
    fn test_jsonb_postgres_bytes_roundtrip() {
        let value = json!({"name": "Alice", "age": 30});
        let container = JsonbContainer::new(value.clone());

        // Encode to bytes
        let bytes = container.to_postgres_bytes().unwrap();

        // Decode from bytes
        let decoded = JsonbContainer::from_postgres_bytes(&bytes).unwrap();

        assert_eq!(decoded.to_json_value(), &value);
    }

    #[test]
    fn test_jsonb_array() {
        let value = json!([1, 2, 3, "four"]);
        let container = JsonbContainer::new(value.clone());

        let bytes = container.to_postgres_bytes().unwrap();
        let decoded = JsonbContainer::from_postgres_bytes(&bytes).unwrap();

        assert_eq!(decoded.to_json_value(), &value);
    }

    #[test]
    fn test_jsonb_scalar() {
        let value = json!("hello");
        let container = JsonbContainer::new(value.clone());

        let bytes = container.to_postgres_bytes().unwrap();
        let decoded = JsonbContainer::from_postgres_bytes(&bytes).unwrap();

        assert_eq!(decoded.to_json_value(), &value);
    }

    #[test]
    fn test_jsonb_null() {
        let value = json!(null);
        let container = JsonbContainer::new(value.clone());

        let bytes = container.to_postgres_bytes().unwrap();
        let decoded = JsonbContainer::from_postgres_bytes(&bytes).unwrap();

        assert_eq!(decoded.to_json_value(), &value);
    }
}
