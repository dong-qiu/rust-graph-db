use chrono::Utc;
use rust_graph_db::{GraphStorage, Graphid};
use serde_json::json;
use std::sync::Arc;

use crate::{
    error::{ApiError, ApiResult},
    models::{CreatePostDto, Post, User},
};

pub struct PostRepository {
    storage: Arc<dyn GraphStorage>,
}

impl PostRepository {
    pub fn new(storage: Arc<dyn GraphStorage>) -> Self {
        Self { storage }
    }

    /// Create a new post
    pub async fn create_post(
        &self,
        user_id: Graphid,
        dto: CreatePostDto,
    ) -> ApiResult<Post> {
        // Create post vertex
        let properties = json!({
            "content": dto.content,
            "created_at": Utc::now().timestamp(),
            "visibility": dto.visibility.unwrap_or_else(|| "public".to_string()),
            "media_url": dto.media_url,
        });

        let post_vertex = self
            .storage
            .create_vertex("Post", properties)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

        // Create POSTED edge from user to post
        let edge_properties = json!({
            "posted_at": Utc::now().timestamp(),
        });

        self.storage
            .create_edge("POSTED", user_id, post_vertex.id, edge_properties)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

        Post::from_json(post_vertex.id, &post_vertex.properties)
            .map_err(|e| ApiError::Internal(e.to_string()))
    }

    /// Get a post by ID
    pub async fn get_post(&self, post_id: Graphid) -> ApiResult<Option<Post>> {
        let vertex = self
            .storage
            .get_vertex(post_id)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

        match vertex {
            Some(v) if v.label == "Post" => {
                let post = Post::from_json(v.id, &v.properties)
                    .map_err(|e| ApiError::Internal(e.to_string()))?;
                Ok(Some(post))
            }
            _ => Ok(None),
        }
    }

    /// Delete a post
    pub async fn delete_post(&self, post_id: Graphid) -> ApiResult<()> {
        // Delete all outgoing edges from post
        let outgoing = self
            .storage
            .get_outgoing_edges(post_id)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

        for edge in outgoing {
            self.storage
                .delete_edge(edge.id)
                .await
                .map_err(|e| ApiError::Database(e.to_string()))?;
        }

        // Delete all incoming edges to post
        let incoming = self
            .storage
            .get_incoming_edges(post_id)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

        for edge in incoming {
            self.storage
                .delete_edge(edge.id)
                .await
                .map_err(|e| ApiError::Database(e.to_string()))?;
        }

        // Delete the post vertex
        self.storage
            .delete_vertex(post_id)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

        Ok(())
    }

    /// Get author of a post
    pub async fn get_post_author(&self, post_id: Graphid) -> ApiResult<Option<User>> {
        let incoming = self
            .storage
            .get_incoming_edges(post_id)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

        for edge in incoming {
            if edge.label == "POSTED" {
                if let Some(vertex) = self
                    .storage
                    .get_vertex(edge.start)
                    .await
                    .map_err(|e| ApiError::Database(e.to_string()))?
                {
                    let user = User::from_json(vertex.id, &vertex.properties)
                        .map_err(|e| ApiError::Internal(e.to_string()))?;
                    return Ok(Some(user));
                }
            }
        }

        Ok(None)
    }

    /// Get posts by a user
    pub async fn get_user_posts(&self, user_id: Graphid, limit: usize) -> ApiResult<Vec<Post>> {
        let outgoing = self
            .storage
            .get_outgoing_edges(user_id)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

        let mut posts = Vec::new();
        for edge in outgoing {
            if edge.label == "POSTED" {
                if let Some(vertex) = self
                    .storage
                    .get_vertex(edge.end)
                    .await
                    .map_err(|e| ApiError::Database(e.to_string()))?
                {
                    let post = Post::from_json(vertex.id, &vertex.properties)
                        .map_err(|e| ApiError::Internal(e.to_string()))?;
                    posts.push(post);

                    if posts.len() >= limit {
                        break;
                    }
                }
            }
        }

        // Sort by created_at descending
        posts.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(posts)
    }

    /// Like a post
    pub async fn like_post(&self, user_id: Graphid, post_id: Graphid) -> ApiResult<()> {
        // Check if already liked
        let outgoing = self
            .storage
            .get_outgoing_edges(user_id)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

        for edge in outgoing {
            if edge.label == "LIKES" && edge.end == post_id {
                return Err(ApiError::AlreadyExists(
                    "Already liked this post".to_string(),
                ));
            }
        }

        // Create LIKES edge
        let properties = json!({
            "liked_at": Utc::now().timestamp(),
        });

        self.storage
            .create_edge("LIKES", user_id, post_id, properties)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

        Ok(())
    }

    /// Unlike a post
    pub async fn unlike_post(&self, user_id: Graphid, post_id: Graphid) -> ApiResult<()> {
        let outgoing = self
            .storage
            .get_outgoing_edges(user_id)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

        for edge in outgoing {
            if edge.label == "LIKES" && edge.end == post_id {
                self.storage
                    .delete_edge(edge.id)
                    .await
                    .map_err(|e| ApiError::Database(e.to_string()))?;
                return Ok(());
            }
        }

        Err(ApiError::NotFound("Haven't liked this post".to_string()))
    }

    /// Get users who liked a post
    pub async fn get_post_likes(&self, post_id: Graphid) -> ApiResult<Vec<User>> {
        let incoming = self
            .storage
            .get_incoming_edges(post_id)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

        let mut users = Vec::new();
        for edge in incoming {
            if edge.label == "LIKES" {
                if let Some(vertex) = self
                    .storage
                    .get_vertex(edge.start)
                    .await
                    .map_err(|e| ApiError::Database(e.to_string()))?
                {
                    let user = User::from_json(vertex.id, &vertex.properties)
                        .map_err(|e| ApiError::Internal(e.to_string()))?;
                    users.push(user);
                }
            }
        }

        Ok(users)
    }
}
