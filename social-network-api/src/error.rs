use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Resource already exists: {0}")]
    AlreadyExists(String),

    #[error("Invalid input: {0}")]
    BadRequest(String),

    #[error("Authentication failed")]
    Unauthorized,

    #[error("Permission denied")]
    Forbidden,

    #[error("Database error: {0}")]
    Database(String),

    #[error("Internal server error: {0}")]
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            ApiError::AlreadyExists(msg) => (StatusCode::CONFLICT, msg.clone()),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            ApiError::Forbidden => (StatusCode::FORBIDDEN, "Forbidden".to_string()),
            ApiError::Database(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            ApiError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };

        let body = Json(json!({
            "error": error_message,
            "status": status.as_u16(),
        }));

        (status, body).into_response()
    }
}

// Convert rust-graph-db errors to ApiError
impl From<rust_graph_db::StorageError> for ApiError {
    fn from(err: rust_graph_db::StorageError) -> Self {
        ApiError::Database(err.to_string())
    }
}

impl From<rust_graph_db::ExecutionError> for ApiError {
    fn from(err: rust_graph_db::ExecutionError) -> Self {
        ApiError::Database(err.to_string())
    }
}

impl From<rust_graph_db::AlgorithmError> for ApiError {
    fn from(err: rust_graph_db::AlgorithmError) -> Self {
        ApiError::Internal(err.to_string())
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        ApiError::Internal(err.to_string())
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
