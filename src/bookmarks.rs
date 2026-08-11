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
// BOOKMARK POST
// =====================================================================

pub async fn bookmark_post(
    State(state): State<AppState>,
    user: AuthUser,
    Path(post_id): Path<u64>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    // Check if post exists
    let exists: Option<(u64,)> = sqlx::query_as(
        "SELECT id FROM posts WHERE id = ? AND is_deleted = 0"
    )
    .bind(post_id)
    .fetch_optional(&state.db)
    .await?;

    if exists.is_none() {
        return Err(AppError::NotFound("Post not found".to_string()));
    }

    let result = sqlx::query(
        "INSERT IGNORE INTO bookmarks (user_id, post_id) VALUES (?, ?)"
    )
    .bind(user.id)
    .bind(post_id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::Conflict(
            "Post already bookmarked".to_string(),
        ));
    }

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "message": "Post bookmarked"
        })),
    ))
}

// =====================================================================
// UNBOOKMARK POST
// =====================================================================

pub async fn unbookmark_post(
    State(state): State<AppState>,
    user: AuthUser,
    Path(post_id): Path<u64>,
) -> AppResult<Json<serde_json::Value>> {
    let result = sqlx::query(
        "DELETE FROM bookmarks WHERE user_id = ? AND post_id = ?"
    )
    .bind(user.id)
    .bind(post_id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(
            "Bookmark not found".to_string(),
        ));
    }

    Ok(Json(json!({
        "message": "Bookmark removed"
    })))
}

// =====================================================================
// GET USER'S BOOKMARKS
// =====================================================================

pub async fn get_user_bookmarks(
    State(state): State<AppState>,
    user: AuthUser,
) -> AppResult<Json<Vec<crate::models::PostResponse>>> {
    let rows: Vec<crate::models::PostFeedRow> = sqlx::query_as(
        "SELECT p.id, p.user_id, u.username, u.display_name, u.avatar_url, \
         c.short_tag as college_short_tag, p.body, p.image_path, \
         p.upvotes_count, p.downvotes_count, p.comments_count, p.reposts_count, \
         p.hot_score, p.created_at \
         FROM posts p \
         JOIN users u ON u.id = p.user_id \
         JOIN colleges c ON c.id = p.college_id \
         JOIN bookmarks b ON b.post_id = p.id \
         WHERE b.user_id = ? AND p.is_deleted = 0 \
         ORDER BY b.created_at DESC \
         LIMIT 100"
    )
    .bind(user.id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows.into_iter().map(crate::models::PostResponse::from).collect()))
}
