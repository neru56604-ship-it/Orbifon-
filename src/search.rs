use crate::{
    error::{AppError, AppResult},
    models::*,
    AppState,
};
use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    #[serde(rename = "type")]
    pub search_type: Option<String>, // "posts", "users", "hashtags"
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// =====================================================================
// SEARCH POSTS BY CONTENT
// =====================================================================

pub async fn search_posts(
    State(state): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> AppResult<Json<Vec<PostResponse>>> {
    if q.q.is_empty() || q.q.len() > 500 {
        return Err(AppError::Validation(
            "Search query must be 1-500 characters".to_string(),
        ));
    }

    let limit = q.limit.unwrap_or(25).min(100);
    let offset = q.offset.unwrap_or(0);
    let search_term = format!("%{}%", q.q);

    let rows: Vec<PostFeedRow> = sqlx::query_as(
        "SELECT p.id, p.user_id, u.username, u.display_name, u.avatar_url, \
         c.short_tag as college_short_tag, p.body, p.image_path, \
         p.upvotes_count, p.downvotes_count, p.comments_count, p.reposts_count, \
         p.hot_score, p.created_at \
         FROM posts p \
         JOIN users u ON u.id = p.user_id \
         JOIN colleges c ON c.id = p.college_id \
         WHERE p.is_deleted = 0 AND (p.body LIKE ? OR p.id IN \
            (SELECT post_id FROM hashtags WHERE tag LIKE ?)) \
         ORDER BY p.created_at DESC \
         LIMIT ? OFFSET ?"
    )
    .bind(&search_term)
    .bind(q.q.trim_start_matches('#').to_lowercase())
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows.into_iter().map(PostResponse::from).collect()))
}

// =====================================================================
// SEARCH USERS BY USERNAME/DISPLAY NAME
// =====================================================================

pub async fn search_users(
    State(state): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> AppResult<Json<Vec<serde_json::Value>>> {
    if q.q.is_empty() || q.q.len() > 100 {
        return Err(AppError::Validation(
            "Search query must be 1-100 characters".to_string(),
        ));
    }

    let limit = q.limit.unwrap_or(25).min(100);
    let offset = q.offset.unwrap_or(0);
    let search_term = format!("%{}%", q.q);

    let users: Vec<(u64, String, String, Option<String>)> = sqlx::query_as(
        "SELECT id, username, display_name, avatar_url \
         FROM users \
         WHERE is_active = 1 AND (username LIKE ? OR display_name LIKE ?) \
         ORDER BY username ASC \
         LIMIT ? OFFSET ?"
    )
    .bind(&search_term)
    .bind(&search_term)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    let response = users
        .into_iter()
        .map(|(id, username, display_name, avatar_url)| {
            serde_json::json!({
                "id": id,
                "username": username,
                "display_name": display_name,
                "avatar_url": avatar_url,
            })
        })
        .collect();

    Ok(Json(response))
}

// =====================================================================
// SEARCH HASHTAGS
// =====================================================================

pub async fn search_hashtags(
    State(state): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> AppResult<Json<Vec<serde_json::Value>>> {
    if q.q.is_empty() || q.q.len() > 100 {
        return Err(AppError::Validation(
            "Search query must be 1-100 characters".to_string(),
        ));
    }

    let limit = q.limit.unwrap_or(25).min(100);
    let offset = q.offset.unwrap_or(0);
    let search_term = format!("{}%", q.q.trim_start_matches('#').to_lowercase());

    let tags: Vec<(String, u32)> = sqlx::query_as(
        "SELECT tag, COUNT(DISTINCT post_id) as post_count \
         FROM hashtags \
         WHERE tag LIKE ? \
         GROUP BY tag \
         ORDER BY post_count DESC \
         LIMIT ? OFFSET ?"
    )
    .bind(&search_term)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    let response = tags
        .into_iter()
        .map(|(tag, count)| {
            serde_json::json!({
                "tag": tag,
                "post_count": count,
            })
        })
        .collect();

    Ok(Json(response))
}

// =====================================================================
// UNIVERSAL SEARCH
// =====================================================================

pub async fn universal_search(
    State(state): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> AppResult<Json<serde_json::Value>> {
    if q.q.is_empty() || q.q.len() > 500 {
        return Err(AppError::Validation(
            "Search query must be 1-500 characters".to_string(),
        ));
    }

    let limit = 10;
    let search_term = format!("%{}%", q.q);

    // Search posts
    let posts: Vec<PostFeedRow> = sqlx::query_as(
        "SELECT p.id, p.user_id, u.username, u.display_name, u.avatar_url, \
         c.short_tag as college_short_tag, p.body, p.image_path, \
         p.upvotes_count, p.downvotes_count, p.comments_count, p.reposts_count, \
         p.hot_score, p.created_at \
         FROM posts p \
         JOIN users u ON u.id = p.user_id \
         JOIN colleges c ON c.id = p.college_id \
         WHERE p.is_deleted = 0 AND p.body LIKE ? \
         ORDER BY p.created_at DESC \
         LIMIT ?"
    )
    .bind(&search_term)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;

    // Search users
    let users: Vec<(u64, String, String, Option<String>)> = sqlx::query_as(
        "SELECT id, username, display_name, avatar_url \
         FROM users \
         WHERE is_active = 1 AND (username LIKE ? OR display_name LIKE ?) \
         ORDER BY username ASC \
         LIMIT ?"
    )
    .bind(&search_term)
    .bind(&search_term)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;

    // Search hashtags
    let tags: Vec<(String, u32)> = sqlx::query_as(
        "SELECT tag, COUNT(DISTINCT post_id) as post_count \
         FROM hashtags \
         WHERE tag LIKE ? \
         GROUP BY tag \
         ORDER BY post_count DESC \
         LIMIT ?"
    )
    .bind(q.q.trim_start_matches('#').to_lowercase())
    .bind(limit)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(serde_json::json!({
        "posts": posts.into_iter().map(PostResponse::from).collect::<Vec<_>>(),
        "users": users.into_iter().map(|(id, username, display_name, avatar_url)| {
            serde_json::json!({
                "id": id,
                "username": username,
                "display_name": display_name,
                "avatar_url": avatar_url,
            })
        }).collect::<Vec<_>>(),
        "hashtags": tags.into_iter().map(|(tag, count)| {
            serde_json::json!({
                "tag": tag,
                "post_count": count,
            })
        }).collect::<Vec<_>>(),
    })))
}
