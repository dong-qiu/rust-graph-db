use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Error types for Graphid operations
#[derive(Error, Debug)]
pub enum GraphidError {
    #[error("Local ID {0} is out of range (max: 2^48 - 1)")]
    LocidOutOfRange(u64),

    #[error("Invalid Graphid value: {0}")]
    InvalidValue(u64),
}

/// Graphid: 64-bit identifier for graph vertices and edges
///
/// Format: [16-bit label ID][48-bit local ID]
/// - High 16 bits: Label ID (type identifier)
/// - Low 48 bits: Local ID (unique within label)
///
/// This matches the openGauss-graph implementation in src/include/utils/graph.h
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub struct Graphid(u64);

impl Graphid {
    /// Maximum value for local ID (2^48 - 1)
    pub const MAX_LOCID: u64 = 0x0000FFFFFFFFFFFF;

    /// Maximum value for label ID (2^16 - 1)
    pub const MAX_LABID: u16 = u16::MAX;

    /// Create a new Graphid from label ID and local ID
    ///
    /// # Arguments
    /// * `labid` - Label ID (16 bits)
    /// * `locid` - Local ID (48 bits)
    ///
    /// # Returns
    /// * `Ok(Graphid)` if locid is within valid range
    /// * `Err(GraphidError)` if locid exceeds 48 bits
    pub fn new(labid: u16, locid: u64) -> Result<Self, GraphidError> {
        if locid > Self::MAX_LOCID {
            return Err(GraphidError::LocidOutOfRange(locid));
        }
        Ok(Self(((labid as u64) << 48) | locid))
    }

    /// Create a Graphid from raw 64-bit value (unchecked)
    ///
    /// # Safety
    /// This function does not validate the value. Use with caution.
    pub fn from_raw(value: u64) -> Self {
        Self(value)
    }

    /// Get the raw 64-bit value
    pub fn as_raw(&self) -> u64 {
        self.0
    }

    /// Extract label ID (high 16 bits)
    pub fn labid(&self) -> u16 {
        (self.0 >> 48) as u16
    }

    /// Extract local ID (low 48 bits)
    pub fn locid(&self) -> u64 {
        self.0 & Self::MAX_LOCID
    }

    /// Check if this is a valid Graphid
    pub fn is_valid(&self) -> bool {
        self.locid() <= Self::MAX_LOCID
    }
}

impl fmt::Display for Graphid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.labid(), self.locid())
    }
}

impl From<Graphid> for u64 {
    fn from(id: Graphid) -> u64 {
        id.0
    }
}

impl TryFrom<u64> for Graphid {
    type Error = GraphidError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        let id = Self(value);
        if id.is_valid() {
            Ok(id)
        } else {
            Err(GraphidError::InvalidValue(value))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graphid_creation() {
        let id = Graphid::new(1, 100).unwrap();
        assert_eq!(id.labid(), 1);
        assert_eq!(id.locid(), 100);
    }

    #[test]
    fn test_graphid_max_values() {
        let id = Graphid::new(Graphid::MAX_LABID, Graphid::MAX_LOCID).unwrap();
        assert_eq!(id.labid(), Graphid::MAX_LABID);
        assert_eq!(id.locid(), Graphid::MAX_LOCID);
    }

    #[test]
    fn test_graphid_out_of_range() {
        let result = Graphid::new(1, Graphid::MAX_LOCID + 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_graphid_raw_conversion() {
        let original = Graphid::new(5, 12345).unwrap();
        let raw = original.as_raw();
        let restored = Graphid::from_raw(raw);
        assert_eq!(original, restored);
    }

    #[test]
    fn test_graphid_display() {
        let id = Graphid::new(10, 500).unwrap();
        assert_eq!(format!("{}", id), "10.500");
    }

    #[test]
    fn test_graphid_bitwise_structure() {
        // Test that label ID is in high 16 bits and local ID in low 48 bits
        let labid: u16 = 0xABCD;
        let locid: u64 = 0x123456789ABC;
        let id = Graphid::new(labid, locid).unwrap();

        let expected = ((labid as u64) << 48) | locid;
        assert_eq!(id.as_raw(), expected);
    }
}
