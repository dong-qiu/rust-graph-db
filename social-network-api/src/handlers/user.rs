use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::{
    error::ApiResult,
    models::{CreateUserDto, UpdateUserDto, UserResponse},
    AppState,
};

pub async fn create_user(
    State(state): State<AppState>,
    Json(payload): Json<CreateUserDto>,
) -> ApiResult<(StatusCode, Json<UserResponse>)> {
    let user = state.user_service.create_user(payload).await?;
    Ok((StatusCode::CREATED, Json(user.into())))
}

pub async fn get_user(
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> ApiResult<Json<UserResponse>> {
    let user = state.user_service.get_user(&username).await?;
    Ok(Json(user.into()))
}

pub async fn update_user(
    State(state): State<AppState>,
    Path(username): Path<String>,
    Json(payload): Json<UpdateUserDto>,
) -> ApiResult<Json<UserResponse>> {
    let user = state.user_service.update_user(&username, payload).await?;
    Ok(Json(user.into()))
}

pub async fn delete_user(
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> ApiResult<StatusCode> {
    state.user_service.delete_user(&username).await?;
    Ok(StatusCode::NO_CONTENT)
}
