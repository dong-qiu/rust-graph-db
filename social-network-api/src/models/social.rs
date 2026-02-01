use serde::Serialize;

use super::UserResponse;

#[derive(Debug, Serialize)]
pub struct UserWithScore {
    #[serde(flatten)]
    pub user: UserResponse,
    pub mutual_friends: usize,
}

#[derive(Debug, Serialize)]
pub struct NetworkAnalysis {
    pub path: Vec<String>,
    pub degrees_of_separation: usize,
    pub mutual_friends: Vec<UserResponse>,
}
