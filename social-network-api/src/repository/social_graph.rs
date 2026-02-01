use chrono::Utc;
use rust_graph_db::{GraphStorage, Graphid};
use serde_json::json;
use std::{collections::HashSet, sync::Arc};

use crate::{
    error::{ApiError, ApiResult},
    models::User,
};

pub struct SocialGraphRepository {
    pub(crate) storage: Arc<dyn GraphStorage>,
}

impl SocialGraphRepository {
    pub fn new(storage: Arc<dyn GraphStorage>) -> Self {
        Self { storage }
    }

    /// Create a FOLLOWS relationship
    pub async fn follow_user(&self, follower_id: Graphid, followee_id: Graphid) -> ApiResult<()> {
        // Check if already following
        let outgoing = self
            .storage
            .get_outgoing_edges(follower_id)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

        for edge in outgoing {
            if edge.label == "FOLLOWS" && edge.end == followee_id {
                return Err(ApiError::AlreadyExists(
                    "Already following this user".to_string(),
                ));
            }
        }

        // Create the FOLLOWS edge
        let properties = json!({
            "followed_at": Utc::now().timestamp(),
        });

        self.storage
            .create_edge("FOLLOWS", follower_id, followee_id, properties)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

        Ok(())
    }

    /// Remove a FOLLOWS relationship
    pub async fn unfollow_user(
        &self,
        follower_id: Graphid,
        followee_id: Graphid,
    ) -> ApiResult<()> {
        let outgoing = self
            .storage
            .get_outgoing_edges(follower_id)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

        for edge in outgoing {
            if edge.label == "FOLLOWS" && edge.end == followee_id {
                self.storage
                    .delete_edge(edge.id)
                    .await
                    .map_err(|e| ApiError::Database(e.to_string()))?;
                return Ok(());
            }
        }

        Err(ApiError::NotFound("Not following this user".to_string()))
    }

    /// Get followers of a user
    pub async fn get_followers(&self, user_id: Graphid, limit: usize) -> ApiResult<Vec<User>> {
        let incoming = self
            .storage
            .get_incoming_edges(user_id)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

        let mut users = Vec::new();
        for edge in incoming {
            if edge.label == "FOLLOWS" {
                if let Some(vertex) = self
                    .storage
                    .get_vertex(edge.start)
                    .await
                    .map_err(|e| ApiError::Database(e.to_string()))?
                {
                    let user = User::from_json(vertex.id, &vertex.properties)
                        .map_err(|e| ApiError::Internal(e.to_string()))?;
                    users.push(user);

                    if users.len() >= limit {
                        break;
                    }
                }
            }
        }

        Ok(users)
    }

    /// Get users that a user is following
    pub async fn get_following(&self, user_id: Graphid, limit: usize) -> ApiResult<Vec<User>> {
        let outgoing = self
            .storage
            .get_outgoing_edges(user_id)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

        let mut users = Vec::new();
        for edge in outgoing {
            if edge.label == "FOLLOWS" {
                if let Some(vertex) = self
                    .storage
                    .get_vertex(edge.end)
                    .await
                    .map_err(|e| ApiError::Database(e.to_string()))?
                {
                    let user = User::from_json(vertex.id, &vertex.properties)
                        .map_err(|e| ApiError::Internal(e.to_string()))?;
                    users.push(user);

                    if users.len() >= limit {
                        break;
                    }
                }
            }
        }

        Ok(users)
    }

    /// Get mutual friends between two users
    pub async fn get_mutual_friends(
        &self,
        user1_id: Graphid,
        user2_id: Graphid,
    ) -> ApiResult<Vec<User>> {
        let following1 = self.get_following_ids(user1_id).await?;
        let following2 = self.get_following_ids(user2_id).await?;

        let mutual: HashSet<_> = following1.intersection(&following2).cloned().collect();

        let mut users = Vec::new();
        for id in mutual {
            if let Some(vertex) = self
                .storage
                .get_vertex(id)
                .await
                .map_err(|e| ApiError::Database(e.to_string()))?
            {
                let user = User::from_json(vertex.id, &vertex.properties)
                    .map_err(|e| ApiError::Internal(e.to_string()))?;
                users.push(user);
            }
        }

        Ok(users)
    }

    /// Get IDs of users that a user is following
    pub async fn get_following_ids(&self, user_id: Graphid) -> ApiResult<HashSet<Graphid>> {
        let outgoing = self
            .storage
            .get_outgoing_edges(user_id)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

        let ids: HashSet<Graphid> = outgoing
            .into_iter()
            .filter(|e| e.label == "FOLLOWS")
            .map(|e| e.end)
            .collect();

        Ok(ids)
    }
}
