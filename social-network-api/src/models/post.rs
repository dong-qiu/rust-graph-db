use chrono::{DateTime, Utc};
use rust_graph_db::types::Graphid;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::UserResponse;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    pub id: Graphid,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub visibility: String,
    pub media_url: Option<String>,
}

impl Post {
    /// Convert from a JSON vertex value
    pub fn from_json(id: Graphid, properties: &Value) -> anyhow::Result<Self> {
        Ok(Self {
            id,
            content: properties["content"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing content"))?
                .to_string(),
            created_at: DateTime::from_timestamp(
                properties["created_at"]
                    .as_i64()
                    .ok_or_else(|| anyhow::anyhow!("Missing created_at"))?,
                0,
            )
            .ok_or_else(|| anyhow::anyhow!("Invalid timestamp"))?,
            visibility: properties["visibility"]
                .as_str()
                .unwrap_or("public")
                .to_string(),
            media_url: properties["media_url"].as_str().map(|s| s.to_string()),
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct CreatePostDto {
    pub content: String,
    pub visibility: Option<String>,
    pub media_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PostResponse {
    pub id: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub visibility: String,
    pub media_url: Option<String>,
}

impl From<Post> for PostResponse {
    fn from(post: Post) -> Self {
        Self {
            id: post.id.to_string(),
            content: post.content,
            created_at: post.created_at,
            visibility: post.visibility,
            media_url: post.media_url,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PostWithAuthor {
    #[serde(flatten)]
    pub post: PostResponse,
    pub author: UserResponse,
}
