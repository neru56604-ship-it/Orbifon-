use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    models::*,
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

// =====================================================================
// USER PROFILE MODELS
// =====================================================================

#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct UserProfile {
    pub id: u64,
    pub username: String,
    pub display_name: String,
    pub email: String,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub cover_image_url: Option<String>,
    pub follower_count: u32,
    pub following_count: u32,
    pub post_count: u32,
    pub reputation_score: i32,
    pub is_verified: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProfileRequest {
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub cover_image_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProfileQuery {
    pub include_posts: Option<bool>,
}

// =====================================================================
// GET USER PROFILE
// =====================================================================

pub async fn get_user_profile(
    State(state): State<AppState>,
    Path(username): Path<String>,
    Query(q): Query<ProfileQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let user: Option<UserProfile> = sqlx::query_as(
        "SELECT u.id, u.username, u.display_name, u.email, u.bio, u.avatar_url, \
         u.cover_image_url, \
         COALESCE((SELECT COUNT(*) FROM follows WHERE following_id = u.id), 0) as follower_count, \
         COALESCE((SELECT COUNT(*) FROM follows WHERE follower_id = u.id), 0) as following_count, \
         COALESCE((SELECT COUNT(*) FROM posts WHERE user_id = u.id AND is_deleted = 0), 0) as post_count, \
         u.reputation_score, u.is_verified, u.created_at \
         FROM users u \
         WHERE u.username = ? AND u.is_active = 1"
    )
    .bind(&username)
    .fetch_optional(&state.db)
    .await?;

    let user = user.ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let mut response = serde_json::json!({
        "id": user.id,
        "username": user.username,
        "display_name": user.display_name,
        "bio": user.bio,
        "avatar_url": user.avatar_url,
        "cover_image_url": user.cover_image_url,
        "follower_count": user.follower_count,
        "following_count": user.following_count,
        "post_count": user.post_count,
        "reputation_score": user.reputation_score,
        "is_verified": user.is_verified,
        "created_at": user.created_at,
    });

    if q.include_posts.unwrap_or(false) {
        let posts = sqlx::query_as::<_, PostFeedRow>(
            "SELECT p.id, p.user_id, u.username, u.display_name, u.avatar_url, \
             c.short_tag as college_short_tag, p.body, p.image_path, \
             p.upvotes_count, p.downvotes_count, p.comments_count, p.reposts_count, \
             p.hot_score, p.created_at \
             FROM posts p \
             JOIN users u ON u.id = p.user_id \
             JOIN colleges c ON c.id = p.college_id \
             WHERE p.user_id = ? AND p.is_deleted = 0 \
             ORDER BY p.created_at DESC LIMIT 20"
        )
        .bind(user.id)
        .fetch_all(&state.db)
        .await?;

        response["posts"] = serde_json::to_value(posts)?;
    }

    Ok(Json(response))
}

// =====================================================================
// UPDATE USER PROFILE
// =====================================================================

pub async fn update_user_profile(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<UpdateProfileRequest>,
) -> AppResult<Json<serde_json::Value>> {
    // Validate input
    if let Some(ref name) = req.display_name {
        if name.trim().is_empty() || name.len() > 60 {
            return Err(AppError::Validation(
                "Display name must be 1-60 characters".to_string(),
            ));
        }
    }

    if let Some(ref bio) = req.bio {
        if bio.len() > 500 {
            return Err(AppError::Validation(
                "Bio must be max 500 characters".to_string(),
            ));
        }
    }

    sqlx::query(
        "UPDATE users SET display_name = COALESCE(?, display_name), \
         bio = COALESCE(?, bio), \
         avatar_url = COALESCE(?, avatar_url), \
         cover_image_url = COALESCE(?, cover_image_url) \
         WHERE id = ?"
    )
    .bind(&req.display_name)
    .bind(&req.bio)
    .bind(&req.avatar_url)
    .bind(&req.cover_image_url)
    .bind(user.id)
    .execute(&state.db)
    .await?;

    tracing::info!(user_id = user.id, "Profile updated");

    Ok(Json(serde_json::json!({
        "message": "Profile updated successfully"
    })))
}

// =====================================================================
// GET USER'S FOLLOWERS
// =====================================================================

pub async fn get_user_followers(
    State(state): State<AppState>,
    Path(username): Path<String>,
    Query(q): Query<PaginationQuery>,
) -> AppResult<Json<Vec<serde_json::Value>>> {
    let limit = q.limit.unwrap_or(50).min(100);
    let offset = q.offset.unwrap_or(0);

    let followers: Vec<(u64, String, String, Option<String>)> = sqlx::query_as(
        "SELECT u.id, u.username, u.display_name, u.avatar_url \
         FROM users u \
         JOIN follows f ON f.follower_id = u.id \
         WHERE f.following_id = (SELECT id FROM users WHERE username = ? AND is_active = 1) \
         ORDER BY f.created_at DESC \
         LIMIT ? OFFSET ?"
    )
    .bind(&username)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    let response = followers
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
// GET USER'S FOLLOWING
// =====================================================================

pub async fn get_user_following(
    State(state): State<AppState>,
    Path(username): Path<String>,
    Query(q): Query<PaginationQuery>,
) -> AppResult<Json<Vec<serde_json::Value>>> {
    let limit = q.limit.unwrap_or(50).min(100);
    let offset = q.offset.unwrap_or(0);

    let following: Vec<(u64, String, String, Option<String>)> = sqlx::query_as(
        "SELECT u.id, u.username, u.display_name, u.avatar_url \
         FROM users u \
         JOIN follows f ON f.following_id = u.id \
         WHERE f.follower_id = (SELECT id FROM users WHERE username = ? AND is_active = 1) \
         ORDER BY f.created_at DESC \
         LIMIT ? OFFSET ?"
    )
    .bind(&username)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    let response = following
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
