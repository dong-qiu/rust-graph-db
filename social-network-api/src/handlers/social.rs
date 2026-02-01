use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::{
    error::ApiResult,
    models::{NetworkAnalysis, UserResponse, UserWithScore},
    AppState,
};

#[derive(Deserialize)]
pub struct PaginationParams {
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    50
}

pub async fn follow_user(
    State(state): State<AppState>,
    Path((follower, target)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    state.social_service.follow(&follower, &target).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn unfollow_user(
    State(state): State<AppState>,
    Path((follower, target)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    state.social_service.unfollow(&follower, &target).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_followers(
    State(state): State<AppState>,
    Path(username): Path<String>,
    Query(params): Query<PaginationParams>,
) -> ApiResult<Json<Vec<UserResponse>>> {
    let followers = state.social_service.get_followers(&username, params.limit).await?;
    Ok(Json(followers.into_iter().map(Into::into).collect()))
}

pub async fn get_following(
    State(state): State<AppState>,
    Path(username): Path<String>,
    Query(params): Query<PaginationParams>,
) -> ApiResult<Json<Vec<UserResponse>>> {
    let following = state.social_service.get_following(&username, params.limit).await?;
    Ok(Json(following.into_iter().map(Into::into).collect()))
}

pub async fn get_friend_suggestions(
    State(state): State<AppState>,
    Path(username): Path<String>,
    Query(params): Query<PaginationParams>,
) -> ApiResult<Json<Vec<UserWithScore>>> {
    let suggestions = state.social_service.suggest_friends(&username, params.limit).await?;
    Ok(Json(
        suggestions
            .into_iter()
            .map(|(user, score)| UserWithScore {
                user: user.into(),
                mutual_friends: score,
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
pub struct NetworkParams {
    pub target: String,
}

pub async fn analyze_network(
    State(state): State<AppState>,
    Path(username): Path<String>,
    Query(params): Query<NetworkParams>,
) -> ApiResult<Json<NetworkAnalysis>> {
    let analysis = state.social_service.analyze_network(&username, &params.target).await?;
    Ok(Json(analysis))
}
