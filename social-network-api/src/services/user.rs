use std::sync::Arc;

use crate::{
    error::{ApiError, ApiResult},
    models::{CreateUserDto, UpdateUserDto, User},
    repository::UserRepository,
};

pub struct UserService {
    user_repo: Arc<UserRepository>,
}

impl UserService {
    pub fn new(user_repo: Arc<UserRepository>) -> Self {
        Self { user_repo }
    }

    /// Create a new user
    pub async fn create_user(&self, dto: CreateUserDto) -> ApiResult<User> {
        // Validation
        if dto.username.is_empty() {
            return Err(ApiError::BadRequest("Username cannot be empty".to_string()));
        }
        if dto.email.is_empty() {
            return Err(ApiError::BadRequest("Email cannot be empty".to_string()));
        }
        if dto.password.len() < 6 {
            return Err(ApiError::BadRequest(
                "Password must be at least 6 characters".to_string(),
            ));
        }

        self.user_repo.create_user(dto).await
    }

    /// Get a user by username
    pub async fn get_user(&self, username: &str) -> ApiResult<User> {
        self.user_repo
            .get_user_by_username(username)
            .await?
            .ok_or_else(|| ApiError::NotFound(format!("User '{}' not found", username)))
    }

    /// Update a user
    pub async fn update_user(&self, username: &str, dto: UpdateUserDto) -> ApiResult<User> {
        self.user_repo.update_user(username, dto).await
    }

    /// Delete a user
    pub async fn delete_user(&self, username: &str) -> ApiResult<()> {
        self.user_repo.delete_user(username).await
    }

    /// Authenticate a user
    pub async fn authenticate(&self, username: &str, password: &str) -> ApiResult<User> {
        let is_valid = self.user_repo.verify_password(username, password).await?;

        if !is_valid {
            return Err(ApiError::Unauthorized);
        }

        self.get_user(username).await
    }
}
