use std::sync::Arc;

use crate::{
    error::{ApiError, ApiResult},
    models::{CreatePostDto, Post, PostWithAuthor, User},
    repository::{PostRepository, SocialGraphRepository, UserRepository},
    utils::parse_graphid,
};

pub struct ContentService {
    post_repo: Arc<PostRepository>,
    user_repo: Arc<UserRepository>,
    social_repo: Arc<SocialGraphRepository>,
}

impl ContentService {
    pub fn new(
        post_repo: Arc<PostRepository>,
        user_repo: Arc<UserRepository>,
        social_repo: Arc<SocialGraphRepository>,
    ) -> Self {
        Self {
            post_repo,
            user_repo,
            social_repo,
        }
    }

    /// Create a new post
    pub async fn create_post(&self, username: &str, dto: CreatePostDto) -> ApiResult<Post> {
        // Validation
        if dto.content.is_empty() {
            return Err(ApiError::BadRequest("Content cannot be empty".to_string()));
        }

        let user = self
            .user_repo
            .get_user_by_username(username)
            .await?
            .ok_or_else(|| ApiError::NotFound(format!("User '{}' not found", username)))?;

        self.post_repo.create_post(user.id, dto).await
    }

    /// Get a post
    pub async fn get_post(&self, post_id: &str) -> ApiResult<Post> {
        let id = parse_graphid(post_id)
            .map_err(|e| ApiError::BadRequest(format!("Invalid post ID: {}", e)))?;

        self.post_repo
            .get_post(id)
            .await?
            .ok_or_else(|| ApiError::NotFound(format!("Post '{}' not found", post_id)))
    }

    /// Delete a post
    pub async fn delete_post(&self, post_id: &str) -> ApiResult<()> {
        let id = parse_graphid(post_id)
            .map_err(|e| ApiError::BadRequest(format!("Invalid post ID: {}", e)))?;

        self.post_repo.delete_post(id).await
    }

    /// Get timeline for a user
    pub async fn get_timeline(&self, username: &str, limit: usize) -> ApiResult<Vec<PostWithAuthor>> {
        let user = self
            .user_repo
            .get_user_by_username(username)
            .await?
            .ok_or_else(|| ApiError::NotFound(format!("User '{}' not found", username)))?;

        // Get users that this user follows
        let following = self.social_repo.get_following(user.id, 1000).await?;

        // Get posts from all followed users
        let mut all_posts: Vec<(Post, User)> = Vec::new();

        for followed_user in following {
            let posts = self.post_repo.get_user_posts(followed_user.id, limit).await?;

            for post in posts {
                all_posts.push((post, followed_user.clone()));
            }
        }

        // Sort by created_at descending
        all_posts.sort_by(|a, b| b.0.created_at.cmp(&a.0.created_at));
        all_posts.truncate(limit);

        // Convert to PostWithAuthor
        let timeline: Vec<PostWithAuthor> = all_posts
            .into_iter()
            .map(|(post, author)| PostWithAuthor {
                post: post.into(),
                author: author.into(),
            })
            .collect();

        Ok(timeline)
    }

    /// Like a post
    pub async fn like_post(&self, username: &str, post_id: &str) -> ApiResult<()> {
        let user = self
            .user_repo
            .get_user_by_username(username)
            .await?
            .ok_or_else(|| ApiError::NotFound(format!("User '{}' not found", username)))?;

        let id = parse_graphid(post_id)
            .map_err(|e| ApiError::BadRequest(format!("Invalid post ID: {}", e)))?;

        self.post_repo.like_post(user.id, id).await
    }

    /// Unlike a post
    pub async fn unlike_post(&self, username: &str, post_id: &str) -> ApiResult<()> {
        let user = self
            .user_repo
            .get_user_by_username(username)
            .await?
            .ok_or_else(|| ApiError::NotFound(format!("User '{}' not found", username)))?;

        let id = parse_graphid(post_id)
            .map_err(|e| ApiError::BadRequest(format!("Invalid post ID: {}", e)))?;

        self.post_repo.unlike_post(user.id, id).await
    }

    /// Get users who liked a post
    pub async fn get_post_likes(&self, post_id: &str) -> ApiResult<Vec<User>> {
        let id = parse_graphid(post_id)
            .map_err(|e| ApiError::BadRequest(format!("Invalid post ID: {}", e)))?;

        self.post_repo.get_post_likes(id).await
    }
}
