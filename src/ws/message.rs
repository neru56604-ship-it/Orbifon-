use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WsMessage {
    // Client -> Server
    #[serde(rename = "connect")]
    Connect {
        user_id: u64,
        username: String,
        channel_type: String, // "dm" or "hot_town"
        channel_id: u64,      // user_id for DM, channel_id for hot_town
    },

    #[serde(rename = "message")]
    Message {
        body: String,
        channel_type: String,
        channel_id: u64,
        client_nonce: Option<String>,
    },

    #[serde(rename = "typing")]
    Typing {
        channel_type: String,
        channel_id: u64,
        is_typing: bool,
    },

    #[serde(rename = "read")]
    ReadReceipt {
        channel_type: String,
        channel_id: u64,
        message_id: u64,
    },

    #[serde(rename = "ping")]
    Ping,

    // Server -> Client
    #[serde(rename = "pong")]
    Pong,

    #[serde(rename = "new_message")]
    NewMessage {
        id: u64,
        user_id: u64,
        username: String,
        body: String,
        channel_type: String,
        channel_id: u64,
        created_at: DateTime<Utc>,
        is_deleted: bool,
    },

    #[serde(rename = "user_typing")]
    UserTyping {
        username: String,
        channel_type: String,
        channel_id: u64,
    },

    #[serde(rename = "user_stopped_typing")]
    UserStoppedTyping {
        username: String,
        channel_type: String,
        channel_id: u64,
    },

    #[serde(rename = "read_receipt")]
    ReadReceiptAck {
        message_id: u64,
        read_by: String,
    },

    #[serde(rename = "error")]
    Error { message: String },

    #[serde(rename = "connected")]
    Connected {
        user_id: u64,
        message: String,
    },

    #[serde(rename = "history")]
    History {
        messages: Vec<HistoryMessage>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryMessage {
    pub id: u64,
    pub user_id: u64,
    pub username: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
    pub is_deleted: bool,
}

#[derive(Debug, Clone)]
pub struct ConnectedUser {
    pub user_id: u64,
    pub username: String,
    pub channel_type: String,
    pub channel_id: u64,
}
