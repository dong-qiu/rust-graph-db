use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::{
    error::ApiResult,
    models::{CreatePostDto, PostResponse, PostWithAuthor, UserResponse},
    AppState,
};

#[derive(Deserialize)]
pub struct CreatePostRequest {
    pub username: String,
    #[serde(flatten)]
    pub post: CreatePostDto,
}

#[derive(Deserialize)]
pub struct LikeRequest {
    pub username: String,
}

#[derive(Deserialize)]
pub struct PaginationParams {
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    50
}

pub async fn create_post(
    State(state): State<AppState>,
    Json(payload): Json<CreatePostRequest>,
) -> ApiResult<(StatusCode, Json<PostResponse>)> {
    let post = state.content_service.create_post(&payload.username, payload.post).await?;
    Ok((StatusCode::CREATED, Json(post.into())))
}

pub async fn get_post(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<PostResponse>> {
    let post = state.content_service.get_post(&id).await?;
    Ok(Json(post.into()))
}

pub async fn delete_post(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    state.content_service.delete_post(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_timeline(
    State(state): State<AppState>,
    Path(username): Path<String>,
    Query(params): Query<PaginationParams>,
) -> ApiResult<Json<Vec<PostWithAuthor>>> {
    let timeline = state.content_service.get_timeline(&username, params.limit).await?;
    Ok(Json(timeline))
}

pub async fn like_post(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<LikeRequest>,
) -> ApiResult<StatusCode> {
    state.content_service.like_post(&payload.username, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn unlike_post(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<LikeRequest>,
) -> ApiResult<StatusCode> {
    state.content_service.unlike_post(&payload.username, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_post_likes(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<UserResponse>>> {
    let likes = state.content_service.get_post_likes(&id).await?;
    Ok(Json(likes.into_iter().map(Into::into).collect()))
}
