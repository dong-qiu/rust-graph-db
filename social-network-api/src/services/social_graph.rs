use std::{collections::HashMap, sync::Arc};

use rust_graph_db::{shortest_path, GraphStorage};

use crate::{
    error::{ApiError, ApiResult},
    models::{NetworkAnalysis, User},
    repository::{SocialGraphRepository, UserRepository},
};

pub struct SocialGraphService {
    social_repo: Arc<SocialGraphRepository>,
    user_repo: Arc<UserRepository>,
    storage: Arc<dyn GraphStorage>,
}

impl SocialGraphService {
    pub fn new(
        social_repo: Arc<SocialGraphRepository>,
        user_repo: Arc<UserRepository>,
    ) -> Self {
        // Extract storage from social_repo
        let storage = social_repo.storage.clone();
        Self {
            social_repo,
            user_repo,
            storage,
        }
    }

    /// Follow a user
    pub async fn follow(&self, follower: &str, followee: &str) -> ApiResult<()> {
        // Prevent self-follow
        if follower == followee {
            return Err(ApiError::BadRequest(
                "Cannot follow yourself".to_string(),
            ));
        }

        let follower_user = self.user_repo.get_user_by_username(follower).await?
            .ok_or_else(|| ApiError::NotFound(format!("User '{}' not found", follower)))?;

        let followee_user = self.user_repo.get_user_by_username(followee).await?
            .ok_or_else(|| ApiError::NotFound(format!("User '{}' not found", followee)))?;

        self.social_repo
            .follow_user(follower_user.id, followee_user.id)
            .await
    }

    /// Unfollow a user
    pub async fn unfollow(&self, follower: &str, followee: &str) -> ApiResult<()> {
        let follower_user = self.user_repo.get_user_by_username(follower).await?
            .ok_or_else(|| ApiError::NotFound(format!("User '{}' not found", follower)))?;

        let followee_user = self.user_repo.get_user_by_username(followee).await?
            .ok_or_else(|| ApiError::NotFound(format!("User '{}' not found", followee)))?;

        self.social_repo
            .unfollow_user(follower_user.id, followee_user.id)
            .await
    }

    /// Get followers
    pub async fn get_followers(&self, username: &str, limit: usize) -> ApiResult<Vec<User>> {
        let user = self.user_repo.get_user_by_username(username).await?
            .ok_or_else(|| ApiError::NotFound(format!("User '{}' not found", username)))?;

        self.social_repo.get_followers(user.id, limit).await
    }

    /// Get following
    pub async fn get_following(&self, username: &str, limit: usize) -> ApiResult<Vec<User>> {
        let user = self.user_repo.get_user_by_username(username).await?
            .ok_or_else(|| ApiError::NotFound(format!("User '{}' not found", username)))?;

        self.social_repo.get_following(user.id, limit).await
    }

    /// Suggest friends based on 2-hop neighbors
    pub async fn suggest_friends(&self, username: &str, limit: usize) -> ApiResult<Vec<(User, usize)>> {
        let user = self.user_repo.get_user_by_username(username).await?
            .ok_or_else(|| ApiError::NotFound(format!("User '{}' not found", username)))?;

        // Get users that the current user is following
        let following_ids = self.social_repo.get_following_ids(user.id).await?;

        // Get 2-hop neighbors (friends of friends)
        let mut candidate_scores: HashMap<String, usize> = HashMap::new();

        for friend_id in &following_ids {
            let friends_of_friend = self.social_repo.get_following(*friend_id, 1000).await?;

            for candidate in friends_of_friend {
                // Skip if it's the current user
                if candidate.id == user.id {
                    continue;
                }
                // Skip if already following
                if following_ids.contains(&candidate.id) {
                    continue;
                }

                *candidate_scores.entry(candidate.username.clone()).or_insert(0) += 1;
            }
        }

        // Sort by mutual friends count
        let mut suggestions: Vec<_> = candidate_scores.into_iter().collect();
        suggestions.sort_by(|a, b| b.1.cmp(&a.1));
        suggestions.truncate(limit);

        // Get user details
        let mut results = Vec::new();
        for (username, score) in suggestions {
            if let Some(user) = self.user_repo.get_user_by_username(&username).await? {
                results.push((user, score));
            }
        }

        Ok(results)
    }

    /// Analyze network between two users
    pub async fn analyze_network(&self, user1: &str, user2: &str) -> ApiResult<NetworkAnalysis> {
        let user1_obj = self.user_repo.get_user_by_username(user1).await?
            .ok_or_else(|| ApiError::NotFound(format!("User '{}' not found", user1)))?;

        let user2_obj = self.user_repo.get_user_by_username(user2).await?
            .ok_or_else(|| ApiError::NotFound(format!("User '{}' not found", user2)))?;

        // Find shortest path
        let path_result = shortest_path(
            self.storage.clone(),
            user1_obj.id,
            user2_obj.id,
        )
        .await;

        let path: Vec<String> = if let Ok(result) = path_result {
            let mut names = vec![user1.to_string()];
            for vid in result.path.iter().skip(1) {
                if let Some(v) = self.storage.get_vertex(*vid).await
                    .map_err(|e| ApiError::Database(e.to_string()))? {
                    if let Some(username) = v.properties["username"].as_str() {
                        names.push(username.to_string());
                    }
                }
            }
            names
        } else {
            vec![]
        };

        let degrees = if path.is_empty() { 0 } else { path.len() - 1 };

        // Get mutual friends
        let mutual_friends = self
            .social_repo
            .get_mutual_friends(user1_obj.id, user2_obj.id)
            .await?;

        Ok(NetworkAnalysis {
            path,
            degrees_of_separation: degrees,
            mutual_friends: mutual_friends.into_iter().map(Into::into).collect(),
        })
    }
}
