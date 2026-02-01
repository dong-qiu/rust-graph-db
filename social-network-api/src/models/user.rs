use chrono::{DateTime, Utc};
use rust_graph_db::types::Graphid;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Graphid,
    pub username: String,
    pub email: String,
    pub display_name: String,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl User {
    /// Convert from a JSON vertex value
    pub fn from_json(id: Graphid, properties: &Value) -> anyhow::Result<Self> {
        Ok(Self {
            id,
            username: properties["username"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing username"))?
                .to_string(),
            email: properties["email"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing email"))?
                .to_string(),
            display_name: properties["display_name"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing display_name"))?
                .to_string(),
            bio: properties["bio"].as_str().map(|s| s.to_string()),
            avatar_url: properties["avatar_url"].as_str().map(|s| s.to_string()),
            created_at: DateTime::from_timestamp(
                properties["created_at"]
                    .as_i64()
                    .ok_or_else(|| anyhow::anyhow!("Missing created_at"))?,
                0,
            )
            .ok_or_else(|| anyhow::anyhow!("Invalid timestamp"))?,
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateUserDto {
    pub username: String,
    pub email: String,
    pub display_name: String,
    pub password: String,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserDto {
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: String,
    pub username: String,
    pub email: String,
    pub display_name: String,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id.to_string(),
            username: user.username,
            email: user.email,
            display_name: user.display_name,
            bio: user.bio,
            avatar_url: user.avatar_url,
            created_at: user.created_at,
        }
    }
}
