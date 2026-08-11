use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use rust_decimal::Decimal;
use std::str::FromStr;

// ---------------------------------------------------------------------
// Raw DB rows (sqlx::FromRow) — one struct per table, matching Step 1
// schema exactly. Kept separate from API response DTOs below so the
// wire format can evolve without touching the DB layer.
// ---------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
pub struct CollegeRow {
    pub id: u8,
    pub name: String,
    pub short_tag: String,
    pub email_domain: String,
    pub slug: String,
}

#[derive(Debug, sqlx::FromRow)]
pub struct UserRow {
    pub id: u64,
    pub college_id: u8,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, sqlx::FromRow)]
pub struct PostFeedRow {
    pub id: u64,
    pub user_id: u64,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub college_short_tag: String,
    pub body: Option<String>,
    pub image_path: Option<String>,
    pub upvotes_count: u32,
    pub downvotes_count: u32,
    pub comments_count: u32,
    pub reposts_count: u32,
    pub hot_score: Decimal,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CommentRow {
    pub id: u64,
    pub post_id: u64,
    pub user_id: u64,
    pub username: String,
    pub display_name: String,
    pub parent_comment_id: Option<u64>,
    pub body: String,
    pub upvotes_count: u32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ChannelRow {
    pub id: u16,
    pub server_id: u16,
    pub name: String,
    pub display_label: String,
    pub position: u8,
    pub is_anonymous: bool,
}

#[derive(Debug, sqlx::FromRow)]
pub struct MessageRow {
    pub id: u64,
    pub channel_id: u16,
    pub user_id: u64,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub is_channel_anonymous: bool,
    pub body: String,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------
// API response DTOs
// ---------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct PostResponse {
    pub id: u64,
    pub author_username: String,
    pub author_display_name: String,
    pub author_avatar_url: Option<String>,
    pub college_tag: String,
    pub body: Option<String>,
    pub image_path: Option<String>,
    pub upvotes: u32,
    pub downvotes: u32,
    pub comments: u32,
    pub reposts: u32,
    pub hot_score: f64,
    pub created_at: DateTime<Utc>,
}

impl From<PostFeedRow> for PostResponse {
    fn from(r: PostFeedRow) -> Self {
        PostResponse {
            id: r.id,
            author_username: r.username,
            author_display_name: r.display_name,
            author_avatar_url: r.avatar_url,
            college_tag: r.college_short_tag,
            body: r.body,
            image_path: r.image_path,
            upvotes: r.upvotes_count,
            downvotes: r.downvotes_count,
            comments: r.comments_count,
            reposts: r.reposts_count,
            hot_score: f64::from_str(&r.hot_score.to_string()).unwrap_or(0.0),
            created_at: r.created_at,
        }
    }
}

/// A message as it goes OVER THE WIRE. Note there is no `user_id` field
/// here at all — the masking decision happens once, in `From<MessageRow>`,
/// so no handler can accidentally leak the real author downstream.
#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub id: u64,
    pub channel_id: u16,
    pub author_display_name: String,
    pub author_username: Option<String>, // None when channel is anonymous
    pub author_avatar_url: Option<String>,
    pub body: String,
    pub created_at: DateTime<Utc>,
    pub is_anonymous: bool,
}

impl From<MessageRow> for MessageResponse {
    fn from(r: MessageRow) -> Self {
        if r.is_channel_anonymous {
            let pseudonym = crate::hot_town::confession_alias(r.user_id, r.channel_id);
            MessageResponse {
                id: r.id,
                channel_id: r.channel_id,
                author_display_name: pseudonym,
                author_username: None,
                author_avatar_url: None,
                body: r.body,
                created_at: r.created_at,
                is_anonymous: true,
            }
        } else {
            MessageResponse {
                id: r.id,
                channel_id: r.channel_id,
                author_display_name: r.display_name,
                author_username: Some(r.username),
                author_avatar_url: r.avatar_url,
                body: r.body,
                created_at: r.created_at,
                is_anonymous: false,
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    pub display_name: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub username: String,
    pub college_tag: String,
}

#[derive(Debug, Deserialize)]
pub struct CreatePostRequest {
    pub body: Option<String>,
    pub image_path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct VoteRequest {
    pub vote_type: VoteType,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum VoteType {
    Up,
    Down,
}

impl VoteType {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            VoteType::Up => "up",
            VoteType::Down => "down",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateCommentRequest {
    pub body: String,
    pub parent_comment_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateMessageRequest {
    pub body: String,
    pub client_nonce: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FeedQuery {
    pub sort: Option<String>, // "hot" (default) | "new"
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct MessagesQuery {
    pub since_id: Option<u64>, // long-poll cursor: only messages with id > since_id
    pub limit: Option<u32>,
}
