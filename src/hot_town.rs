use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use sha2::{Digest, Sha256};

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    models::{ChannelRow, CreateMessageRequest, MessageRow, MessageResponse, MessagesQuery},
    AppState,
};

/// Deterministic pseudonym for #confessions: same user always gets the
/// same alias *within a given channel* (so a conversation reads
/// coherently — "Confession #A1B2C3" replying to themselves twice still
/// looks consistent) but there is NO way to derive the real user_id back
/// out of this hash, and the alias differs across channels/servers since
/// channel_id is part of the input.
pub fn confession_alias(user_id: u64, channel_id: u16) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("orbifon-confession-salt:{user_id}:{channel_id}"));
    let digest = hasher.finalize();
    let short = hex::encode(&digest[..3]).to_uppercase();
    format!("Confession #{short}")
}

/// Returns the caller's own Hot Town server (each college has exactly
/// one, auto-provisioned by the Step 1 migration) along with its
/// channels, ordered for the sidebar. This single call is enough for
/// the frontend to build the full Discord-style left sidebar.
pub async fn get_my_server(
    State(state): State<AppState>,
    user: AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    let server: Option<(u16, String)> =
        sqlx::query_as("SELECT id, name FROM hot_town_servers WHERE college_id = ?")
            .bind(user.college_id)
            .fetch_optional(&state.db)
            .await?;

    let (server_id, server_name) =
        server.ok_or_else(|| AppError::NotFound("No Hot Town server for your college".to_string()))?;

    let channels: Vec<ChannelRow> = sqlx::query_as(
        "SELECT id, server_id, name, display_label, position, is_anonymous \
         FROM hot_town_channels WHERE server_id = ? ORDER BY position ASC",
    )
    .bind(server_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(serde_json::json!({
        "server_id": server_id,
        "server_name": server_name,
        "channels": channels,
    })))
}

/// CRITICAL SILOING CHECK: confirms the requested channel belongs to a
/// server owned by the caller's OWN college. This is the guard clause
/// referenced in the architecture overview — every Hot Town read/write
/// route must call this before touching messages. An IIITM student
/// hitting an MITS channel ID gets 403, not data.
async fn assert_channel_access(
    state: &AppState,
    channel_id: u16,
    college_id: u8,
) -> AppResult<ChannelRow> {
    let channel: Option<ChannelRow> = sqlx::query_as(
        "SELECT ch.id, ch.server_id, ch.name, ch.display_label, ch.position, ch.is_anonymous \
         FROM hot_town_channels ch \
         JOIN hot_town_servers s ON s.id = ch.server_id \
         WHERE ch.id = ? AND s.college_id = ?",
    )
    .bind(channel_id)
    .bind(college_id)
    .fetch_optional(&state.db)
    .await?;

    channel.ok_or_else(|| {
        AppError::Forbidden("This channel does not belong to your college's Hot Town".to_string())
    })
}

/// Long-polling-friendly fetch: pass `since_id` (the highest message id
/// you already have) and this returns only newer messages. The frontend
/// can poll this on an interval, or hold the connection with a
/// server-side wait loop later — the contract (since_id in, new
/// messages out) doesn't change either way, which is what keeps this
/// swappable for a WebSocket push later without a frontend rewrite.
pub async fn get_messages(
    State(state): State<AppState>,
    user: AuthUser,
    Path(channel_id): Path<u16>,
    Query(q): Query<MessagesQuery>,
) -> AppResult<Json<Vec<MessageResponse>>> {
    assert_channel_access(&state, channel_id, user.college_id).await?;

    let since_id = q.since_id.unwrap_or(0);
    let limit = q.limit.unwrap_or(50).min(200);

    let rows: Vec<MessageRow> = sqlx::query_as(
        "SELECT m.id, m.channel_id, m.user_id, u.username, u.display_name, u.avatar_url, \
                ch.is_anonymous AS is_channel_anonymous, m.body, m.created_at \
         FROM messages m \
         JOIN users u ON u.id = m.user_id \
         JOIN hot_town_channels ch ON ch.id = m.channel_id \
         WHERE m.channel_id = ? AND m.id > ? AND m.is_deleted = 0 \
         ORDER BY m.id ASC \
         LIMIT ?",
    )
    .bind(channel_id)
    .bind(since_id)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows.into_iter().map(MessageResponse::from).collect()))
}

pub async fn post_message(
    State(state): State<AppState>,
    user: AuthUser,
    Path(channel_id): Path<u16>,
    Json(req): Json<CreateMessageRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    assert_channel_access(&state, channel_id, user.college_id).await?;

    let body = req.body.trim();
    if body.is_empty() || body.len() > 2000 {
        return Err(AppError::Validation(
            "Message must be 1-2000 characters".to_string(),
        ));
    }

    // client_nonce + the UNIQUE(channel_id, client_nonce) constraint make
    // this endpoint safe to retry blindly on a flaky connection.
    let result = sqlx::query(
        "INSERT INTO messages (channel_id, user_id, body, client_nonce) VALUES (?, ?, ?, ?)",
    )
    .bind(channel_id)
    .bind(user.id)
    .bind(body)
    .bind(&req.client_nonce)
    .execute(&state.db)
    .await;

    match result {
        Ok(r) => Ok((
            StatusCode::CREATED,
            Json(serde_json::json!({ "message_id": r.last_insert_id() })),
        )),
        Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => {
            // Same client_nonce already inserted -> this is a safe retry,
            // not an error. Return the existing message id.
            let existing: (u64,) = sqlx::query_as(
                "SELECT id FROM messages WHERE channel_id = ? AND client_nonce = ?",
            )
            .bind(channel_id)
            .bind(&req.client_nonce)
            .fetch_one(&state.db)
            .await?;
            Ok((StatusCode::OK, Json(serde_json::json!({ "message_id": existing.0 }))))
        }
        Err(e) => Err(AppError::Database(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alias_is_deterministic_and_channel_scoped() {
        let a1 = confession_alias(42, 7);
        let a2 = confession_alias(42, 7);
        let a3 = confession_alias(42, 8); // different channel -> different alias
        assert_eq!(a1, a2);
        assert_ne!(a1, a3);
    }
}
