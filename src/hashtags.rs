use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    AppState, tags,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde_json::json;

// =====================================================================
// GET POSTS BY HASHTAG
// =====================================================================

pub async fn get_posts_by_hashtag(
    State(state): State<AppState>,
    Path(tag): Path<String>,
) -> AppResult<Json<Vec<crate::models::PostResponse>>> {
    let tag = tag.trim_start_matches('#').to_lowercase();

    if tag.is_empty() || tag.len() > 50 {
        return Err(AppError::Validation(
            "Hashtag must be 1-50 characters".to_string(),
        ));
    }

    let rows: Vec<crate::models::PostFeedRow> = sqlx::query_as(
        "SELECT p.id, p.user_id, u.username, u.display_name, u.avatar_url, \
         c.short_tag as college_short_tag, p.body, p.image_path, \
         p.upvotes_count, p.downvotes_count, p.comments_count, p.reposts_count, \
         p.hot_score, p.created_at \
         FROM posts p \
         JOIN users u ON u.id = p.user_id \
         JOIN colleges c ON c.id = p.college_id \
         JOIN hashtags h ON h.post_id = p.id \
         WHERE h.tag = ? AND p.is_deleted = 0 \
         ORDER BY p.hot_score DESC, p.created_at DESC \
         LIMIT 100"
    )
    .bind(&tag)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows.into_iter().map(crate::models::PostResponse::from).collect()))
}

// =====================================================================
// GET TRENDING HASHTAGS
// =====================================================================

pub async fn get_trending_hashtags(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<serde_json::Value>>> {
    let tags: Vec<(String, u32)> = sqlx::query_as(
        "SELECT tag, COUNT(DISTINCT post_id) as post_count \
         FROM hashtags \
         WHERE created_at > DATE_SUB(NOW(), INTERVAL 7 DAY) \
         GROUP BY tag \
         ORDER BY post_count DESC \
         LIMIT 20"
    )
    .fetch_all(&state.db)
    .await?;

    let response = tags
        .into_iter()
        .map(|(tag, count)| {
            json!({
                "tag": tag,
                "post_count": count,
            })
        })
        .collect();

    Ok(Json(response))
}
