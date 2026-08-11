use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    models::{
        CommentRow, CreateCommentRequest, CreatePostRequest, FeedQuery, PostFeedRow,
        PostResponse, VoteRequest,
    },
    AppState,
};

// ---------------------------------------------------------------------
// HOT-SCORE ALGORITHM
// Score = Upvotes / (Hours_Since_Post + 2)^1.5
// The "+2" prevents division-by-near-zero on brand-new posts (which
// would otherwise produce an enormous, unstable score for the first
// few minutes of a post's life) while still strongly favoring recency.
// This is recalculated and persisted every time a vote lands, so feed
// reads stay a cheap `ORDER BY hot_score DESC` against an index.
// ---------------------------------------------------------------------
fn compute_hot_score(upvotes: i64, created_at: chrono::DateTime<chrono::Utc>) -> f64 {
    let hours = (chrono::Utc::now() - created_at).num_seconds() as f64 / 3600.0;
    let hours = hours.max(0.0);
    (upvotes.max(0) as f64) / (hours + 2.0).powf(1.5)
}

// ---------------------------------------------------------------------
// Image upload — Phase 1 explicitly excludes video. We validate by
// magic bytes (not just file extension, which is trivially spoofable)
// and cap size via Config.max_image_bytes.
// ---------------------------------------------------------------------
fn sniff_image_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("png")
    } else if bytes.len() > 12 && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

pub async fn upload_image(
    State(state): State<AppState>,
    _user: AuthUser, // must be logged in to upload
    mut multipart: Multipart,
) -> AppResult<Json<serde_json::Value>> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Validation(format!("Malformed upload: {e}")))?
    {
        if field.name() != Some("image") {
            continue;
        }

        let data = field
            .bytes()
            .await
            .map_err(|e| AppError::Validation(format!("Failed reading upload: {e}")))?;

        if data.len() > state.config.max_image_bytes {
            return Err(AppError::Validation(format!(
                "Image exceeds max size of {} bytes",
                state.config.max_image_bytes
            )));
        }

        let ext = sniff_image_type(&data).ok_or_else(|| {
            AppError::Validation(
                "Unsupported file type. Only JPG, PNG, and WEBP images are allowed (no video/GIF in Phase 1)."
                    .to_string(),
            )
        })?;

        let filename = format!("{}.{}", Uuid::new_v4(), ext);
        let path = std::path::Path::new(&state.config.upload_dir).join(&filename);

        tokio::fs::create_dir_all(&state.config.upload_dir)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        tokio::fs::write(&path, &data)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;

        return Ok(Json(serde_json::json!({ "image_path": filename })));
    }

    Err(AppError::Validation(
        "No 'image' field found in multipart upload".to_string(),
    ))
}

// ---------------------------------------------------------------------
// Posts
// ---------------------------------------------------------------------

pub async fn create_post(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<CreatePostRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    if req.body.is_none() && req.image_path.is_none() {
        return Err(AppError::Validation(
            "A post needs text, an image, or both".to_string(),
        ));
    }
    if let Some(body) = &req.body {
        if body.len() > 2000 {
            return Err(AppError::Validation("Post body too long (max 2000 chars)".to_string()));
        }
    }

    let result = sqlx::query(
        "INSERT INTO posts (user_id, college_id, body, image_path) VALUES (?, ?, ?, ?)",
    )
    .bind(user.id)
    .bind(user.college_id)
    .bind(&req.body)
    .bind(&req.image_path)
    .execute(&state.db)
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "post_id": result.last_insert_id() })),
    ))
}

pub async fn get_feed(
    State(state): State<AppState>,
    Query(q): Query<FeedQuery>,
) -> AppResult<Json<Vec<PostResponse>>> {
    let limit = q.limit.unwrap_or(25).min(100);
    let offset = q.offset.unwrap_or(0);
    let order_by = match q.sort.as_deref() {
        Some("new") => "p.created_at DESC",
        _ => "p.hot_score DESC, p.created_at DESC",
    };

    // order_by is one of two hardcoded strings above — never user input
    // directly interpolated, so this is not a SQL-injection surface.
    let sql = format!(
        "SELECT p.id, p.user_id, u.username, u.display_name, u.avatar_url, \
                c.short_tag AS college_short_tag, p.body, p.image_path, \
                p.upvotes_count, p.downvotes_count, p.comments_count, p.reposts_count, \
                p.hot_score, p.created_at \
         FROM posts p \
         JOIN users u ON u.id = p.user_id \
         JOIN colleges c ON c.id = p.college_id \
         WHERE p.is_deleted = 0 \
         ORDER BY {order_by} \
         LIMIT ? OFFSET ?"
    );

    let rows: Vec<PostFeedRow> = sqlx::query_as(&sql)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await?;

    Ok(Json(rows.into_iter().map(PostResponse::from).collect()))
}

// ---------------------------------------------------------------------
// Voting — one vote per user per post (DB UNIQUE constraint backs this
// up). Handles: first vote, switching up<->down, and un-voting (sending
// the same vote type again removes it, toggle-style).
// ---------------------------------------------------------------------

pub async fn vote_post(
    State(state): State<AppState>,
    user: AuthUser,
    Path(post_id): Path<u64>,
    Json(req): Json<VoteRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let mut tx = state.db.begin().await?;

    let post_created_at: Option<(chrono::DateTime<chrono::Utc>,)> =
        sqlx::query_as("SELECT created_at FROM posts WHERE id = ? AND is_deleted = 0")
            .bind(post_id)
            .fetch_optional(&mut *tx)
            .await?;
    let created_at = post_created_at
        .ok_or_else(|| AppError::NotFound("Post not found".to_string()))?
        .0;

    let existing: Option<(String,)> =
        sqlx::query_as("SELECT vote_type FROM votes WHERE post_id = ? AND user_id = ?")
            .bind(post_id)
            .bind(user.id)
            .fetch_optional(&mut *tx)
            .await?;

    let new_vote = req.vote_type.as_db_str();

    match existing {
        None => {
            sqlx::query("INSERT INTO votes (post_id, user_id, vote_type) VALUES (?, ?, ?)")
                .bind(post_id)
                .bind(user.id)
                .bind(new_vote)
                .execute(&mut *tx)
                .await?;
            let col = if new_vote == "up" { "upvotes_count" } else { "downvotes_count" };
            sqlx::query(&format!("UPDATE posts SET {col} = {col} + 1 WHERE id = ?"))
                .bind(post_id)
                .execute(&mut *tx)
                .await?;
        }
        Some((old_vote,)) if old_vote == new_vote => {
            // Same vote sent again -> toggle off (un-vote).
            sqlx::query("DELETE FROM votes WHERE post_id = ? AND user_id = ?")
                .bind(post_id)
                .bind(user.id)
                .execute(&mut *tx)
                .await?;
            let col = if new_vote == "up" { "upvotes_count" } else { "downvotes_count" };
            sqlx::query(&format!(
                "UPDATE posts SET {col} = GREATEST({col} - 1, 0) WHERE id = ?"
            ))
            .bind(post_id)
            .execute(&mut *tx)
            .await?;
        }
        Some((old_vote,)) => {
            // Switching from up->down or down->up.
            sqlx::query("UPDATE votes SET vote_type = ? WHERE post_id = ? AND user_id = ?")
                .bind(new_vote)
                .bind(post_id)
                .bind(user.id)
                .execute(&mut *tx)
                .await?;
            let (dec_col, inc_col) = if old_vote == "up" {
                ("upvotes_count", "downvotes_count")
            } else {
                ("downvotes_count", "upvotes_count")
            };
            sqlx::query(&format!(
                "UPDATE posts SET {dec_col} = GREATEST({dec_col} - 1, 0), {inc_col} = {inc_col} + 1 WHERE id = ?"
            ))
            .bind(post_id)
            .execute(&mut *tx)
            .await?;
        }
    }

    let upvotes: (u32,) = sqlx::query_as("SELECT upvotes_count FROM posts WHERE id = ?")
        .bind(post_id)
        .fetch_one(&mut *tx)
        .await?;
    let new_score = compute_hot_score(upvotes.0 as i64, created_at);

    sqlx::query("UPDATE posts SET hot_score = ? WHERE id = ?")
        .bind(new_score)
        .bind(post_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(Json(serde_json::json!({ "hot_score": new_score })))
}

// ---------------------------------------------------------------------
// Comments
// ---------------------------------------------------------------------

pub async fn create_comment(
    State(state): State<AppState>,
    user: AuthUser,
    Path(post_id): Path<u64>,
    Json(req): Json<CreateCommentRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    if req.body.trim().is_empty() || req.body.len() > 1000 {
        return Err(AppError::Validation(
            "Comment must be 1-1000 characters".to_string(),
        ));
    }

    let mut tx = state.db.begin().await?;

    let post_exists: Option<(u64,)> =
        sqlx::query_as("SELECT id FROM posts WHERE id = ? AND is_deleted = 0")
            .bind(post_id)
            .fetch_optional(&mut *tx)
            .await?;
    if post_exists.is_none() {
        return Err(AppError::NotFound("Post not found".to_string()));
    }

    let result = sqlx::query(
        "INSERT INTO comments (post_id, user_id, parent_comment_id, body) VALUES (?, ?, ?, ?)",
    )
    .bind(post_id)
    .bind(user.id)
    .bind(req.parent_comment_id)
    .bind(&req.body)
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE posts SET comments_count = comments_count + 1 WHERE id = ?")
        .bind(post_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "comment_id": result.last_insert_id() })),
    ))
}

#[axum::debug_handler]
pub async fn get_comments(
    State(state): State<AppState>,
    Path(post_id): Path<u64>,
) -> Result<Json<Vec<CommentRow>>, AppError> {
    let rows: Vec<CommentRow> = sqlx::query_as(
        "SELECT cm.id, cm.post_id, cm.user_id, u.username, u.display_name, \
                cm.parent_comment_id, cm.body, cm.upvotes_count, cm.created_at \
         FROM comments cm \
         JOIN users u ON u.id = cm.user_id \
         WHERE cm.post_id = ? AND cm.is_deleted = 0 \
         ORDER BY cm.created_at ASC",
    )
    .bind(post_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows))
}

// ---------------------------------------------------------------------
// Reposts
// ---------------------------------------------------------------------

pub async fn repost(
    State(state): State<AppState>,
    user: AuthUser,
    Path(post_id): Path<u64>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let mut tx = state.db.begin().await?;

    let post_exists: Option<(u64,)> =
        sqlx::query_as("SELECT id FROM posts WHERE id = ? AND is_deleted = 0")
            .bind(post_id)
            .fetch_optional(&mut *tx)
            .await?;
    if post_exists.is_none() {
        return Err(AppError::NotFound("Post not found".to_string()));
    }

    let inserted = sqlx::query(
        "INSERT IGNORE INTO reposts (user_id, original_post_id) VALUES (?, ?)",
    )
    .bind(user.id)
    .bind(post_id)
    .execute(&mut *tx)
    .await?;

    if inserted.rows_affected() == 0 {
        return Err(AppError::Conflict("You already reposted this".to_string()));
    }

    sqlx::query("UPDATE posts SET reposts_count = reposts_count + 1 WHERE id = ?")
        .bind(post_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok((StatusCode::CREATED, Json(serde_json::json!({ "reposted": true }))))
}
