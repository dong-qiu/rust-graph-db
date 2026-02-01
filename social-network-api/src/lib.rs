pub mod config;
pub mod error;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod repository;
pub mod services;
pub mod utils;

pub use config::Config;
pub use error::{ApiError, ApiResult};

use services::{ContentService, SocialGraphService, UserService};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub user_service: Arc<UserService>,
    pub social_service: Arc<SocialGraphService>,
    pub content_service: Arc<ContentService>,
}
