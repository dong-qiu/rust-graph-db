use chrono::Utc;
use rust_graph_db::{GraphStorage, Graphid};
use serde_json::json;
use std::sync::Arc;

use crate::{
    error::{ApiError, ApiResult},
    models::{CreateUserDto, UpdateUserDto, User},
};

pub struct UserRepository {
    storage: Arc<dyn GraphStorage>,
}

impl UserRepository {
    pub fn new(storage: Arc<dyn GraphStorage>) -> Self {
        Self { storage }
    }

    /// Create a new user
    pub async fn create_user(&self, dto: CreateUserDto) -> ApiResult<User> {
        // Check if username already exists
        if self.find_by_username(&dto.username).await?.is_some() {
            return Err(ApiError::AlreadyExists(format!(
                "User '{}' already exists",
                dto.username
            )));
        }

        // Hash password
        let password_hash = bcrypt::hash(&dto.password, bcrypt::DEFAULT_COST)
            .map_err(|e| ApiError::Internal(format!("Password hashing failed: {}", e)))?;

        // Create vertex
        let properties = json!({
            "username": dto.username,
            "email": dto.email,
            "display_name": dto.display_name,
            "password_hash": password_hash,
            "bio": dto.bio,
            "avatar_url": dto.avatar_url,
            "created_at": Utc::now().timestamp(),
        });

        let vertex = self
            .storage
            .create_vertex("User", properties)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

        User::from_json(vertex.id, &vertex.properties)
            .map_err(|e| ApiError::Internal(e.to_string()))
    }

    /// Get user by username
    pub async fn get_user_by_username(&self, username: &str) -> ApiResult<Option<User>> {
        self.find_by_username(username).await
    }

    /// Get user by ID
    pub async fn get_user_by_id(&self, id: Graphid) -> ApiResult<Option<User>> {
        let vertex = self
            .storage
            .get_vertex(id)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

        match vertex {
            Some(v) => {
                let user = User::from_json(v.id, &v.properties)
                    .map_err(|e| ApiError::Internal(e.to_string()))?;
                Ok(Some(user))
            }
            None => Ok(None),
        }
    }

    /// Update user
    pub async fn update_user(&self, username: &str, dto: UpdateUserDto) -> ApiResult<User> {
        let user = self
            .find_by_username(username)
            .await?
            .ok_or_else(|| ApiError::NotFound(format!("User '{}' not found", username)))?;

        // Get current vertex
        let vertex = self
            .storage
            .get_vertex(user.id)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("User vertex not found")))?;

        // Update properties
        let mut properties = vertex.properties.clone();
        if let Some(email) = dto.email {
            properties["email"] = json!(email);
        }
        if let Some(display_name) = dto.display_name {
            properties["display_name"] = json!(display_name);
        }
        if let Some(bio) = dto.bio {
            properties["bio"] = json!(bio);
        }
        if let Some(avatar_url) = dto.avatar_url {
            properties["avatar_url"] = json!(avatar_url);
        }

        // Delete old vertex and create new one (as storage doesn't have update)
        self.storage
            .delete_vertex(user.id)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

        let new_vertex = self
            .storage
            .create_vertex("User", properties)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

        User::from_json(new_vertex.id, &new_vertex.properties)
            .map_err(|e| ApiError::Internal(e.to_string()))
    }

    /// Delete user
    pub async fn delete_user(&self, username: &str) -> ApiResult<()> {
        let user = self
            .find_by_username(username)
            .await?
            .ok_or_else(|| ApiError::NotFound(format!("User '{}' not found", username)))?;

        // Delete all outgoing edges (FOLLOWS, POSTED, LIKES)
        let outgoing = self
            .storage
            .get_outgoing_edges(user.id)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

        for edge in outgoing {
            self.storage
                .delete_edge(edge.id)
                .await
                .map_err(|e| ApiError::Database(e.to_string()))?;
        }

        // Delete all incoming edges
        let incoming = self
            .storage
            .get_incoming_edges(user.id)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

        for edge in incoming {
            self.storage
                .delete_edge(edge.id)
                .await
                .map_err(|e| ApiError::Database(e.to_string()))?;
        }

        // Delete the vertex
        self.storage
            .delete_vertex(user.id)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

        Ok(())
    }

    /// Verify password
    pub async fn verify_password(&self, username: &str, password: &str) -> ApiResult<bool> {
        let user = self
            .find_by_username(username)
            .await?
            .ok_or_else(|| ApiError::NotFound(format!("User '{}' not found", username)))?;

        let vertex = self
            .storage
            .get_vertex(user.id)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("User vertex not found")))?;

        let password_hash = vertex.properties["password_hash"]
            .as_str()
            .ok_or_else(|| ApiError::Internal("Password hash not found".to_string()))?;

        bcrypt::verify(password, password_hash)
            .map_err(|e| ApiError::Internal(format!("Password verification failed: {}", e)))
    }

    /// Helper: Find user by username
    async fn find_by_username(&self, username: &str) -> ApiResult<Option<User>> {
        let vertices = self
            .storage
            .scan_vertices("User")
            .await;

        // If label doesn't exist yet, no users exist
        let vertices = match vertices {
            Ok(v) => v,
            Err(e) => {
                if e.to_string().contains("Label not found") {
                    return Ok(None);
                }
                return Err(ApiError::Database(e.to_string()));
            }
        };

        for vertex in vertices {
            if let Some(uname) = vertex.properties["username"].as_str() {
                if uname == username {
                    let user = User::from_json(vertex.id, &vertex.properties)
                        .map_err(|e| ApiError::Internal(e.to_string()))?;
                    return Ok(Some(user));
                }
            }
        }

        Ok(None)
    }
}
