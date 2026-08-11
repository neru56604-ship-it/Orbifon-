use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    AppState,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde_json::json;

// =====================================================================
// FOLLOW/UNFOLLOW USER
// =====================================================================

pub async fn follow_user(
    State(state): State<AppState>,
    user: AuthUser,
    Path(target_user_id): Path<u64>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    // Prevent self-follow
    if target_user_id == user.id {
        return Err(AppError::Validation(
            "You cannot follow yourself".to_string(),
        ));
    }

    // Check if target user exists
    let exists: Option<(u64,)> = sqlx::query_as(
        "SELECT id FROM users WHERE id = ? AND is_active = 1"
    )
    .bind(target_user_id)
    .fetch_optional(&state.db)
    .await?;

    if exists.is_none() {
        return Err(AppError::NotFound("User not found".to_string()));
    }

    // Insert follow relationship
    let result = sqlx::query(
        "INSERT IGNORE INTO follows (follower_id, following_id) VALUES (?, ?)"
    )
    .bind(user.id)
    .bind(target_user_id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::Conflict(
            "You already follow this user".to_string(),
        ));
    }

    // Log for audit
    tracing::info!(
        follower_id = user.id,
        following_id = target_user_id,
        "User followed"
    );

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "message": "User followed successfully"
        })),
    ))
}

// =====================================================================
// UNFOLLOW USER
// =====================================================================

pub async fn unfollow_user(
    State(state): State<AppState>,
    user: AuthUser,
    Path(target_user_id): Path<u64>,
) -> AppResult<Json<serde_json::Value>> {
    let result = sqlx::query(
        "DELETE FROM follows WHERE follower_id = ? AND following_id = ?"
    )
    .bind(user.id)
    .bind(target_user_id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(
            "Follow relationship not found".to_string(),
        ));
    }

    tracing::info!(
        follower_id = user.id,
        following_id = target_user_id,
        "User unfollowed"
    );

    Ok(Json(json!({
        "message": "User unfollowed successfully"
    })))
}

// =====================================================================
// CHECK IF FOLLOWING
// =====================================================================

pub async fn is_following(
    State(state): State<AppState>,
    user: AuthUser,
    Path(target_user_id): Path<u64>,
) -> AppResult<Json<serde_json::Value>> {
    let result: Option<(u64,)> = sqlx::query_as(
        "SELECT follower_id FROM follows WHERE follower_id = ? AND following_id = ?"
    )
    .bind(user.id)
    .bind(target_user_id)
    .fetch_optional(&state.db)
    .await?;

    Ok(Json(json!({
        "is_following": result.is_some()
    })))
}
