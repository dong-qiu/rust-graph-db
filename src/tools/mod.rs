/// Data import and export tools
///
/// This module provides utilities for importing and exporting graph data,
/// particularly for migrating from openGauss-graph to the Rust implementation.

pub mod import;
pub mod export;

pub use import::{ImportOptions, ImportStats, import_from_csv, import_from_json};
pub use export::{ExportOptions, ExportFormat, export_to_csv, export_to_json};

use thiserror::Error;

/// Import/export errors
#[derive(Error, Debug)]
pub enum ToolError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Storage error: {0}")]
    StorageError(#[from] crate::storage::StorageError),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("CSV error: {0}")]
    CsvError(#[from] csv::Error),

    #[error("Invalid data format: {0}")]
    InvalidFormat(String),

    #[error("Import failed: {0}")]
    ImportFailed(String),
}

pub type ToolResult<T> = Result<T, ToolError>;
