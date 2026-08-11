use axum::{
    extract::{ws::{WebSocket, WebSocketUpgrade}, State, Path},
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use serde_json::{json, from_str};
use tokio::sync::mpsc;
use chrono::Utc;

use crate::{AppState, auth::AuthUser, error::{AppError, AppResult}, ws::{WsMessage, WsState, ConnectedUser}};

// ============================================================================
// DM WebSocket Handler
// ============================================================================

pub async fn dm_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    user: AuthUser,
) -> impl IntoResponse {
    let ws_state = WsState::new();
    ws.on_upgrade(move |socket| handle_dm_socket(socket, state, user, ws_state))
}

async fn handle_dm_socket(
    socket: WebSocket,
    db_state: AppState,
    auth_user: AuthUser,
    ws_state: WsState,
) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel();

    // Spawn task to forward messages from channel to WebSocket
    let mut send_task = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if sender.send(axum::extract::ws::Message::Text(message)).await.is_err() {
                break;
            }
        }
    });

    let mut recv_task = {
        let ws_state = ws_state.clone();
        let auth_user = auth_user.clone();
        
        tokio::spawn(async move {
            while let Some(Ok(msg)) = receiver.next().await {
                match msg {
                    axum::extract::ws::Message::Text(text) => {
                        if let Ok(ws_msg) = from_str::<WsMessage>(&text) {
                            if let Err(e) = handle_dm_message(&ws_state, &db_state, &auth_user, ws_msg).await {
                                let error_msg = serde_json::to_string(&WsMessage::Error {
                                    message: e.to_string(),
                                }).unwrap();
                                let _ = tx.send(error_msg);
                            }
                        }
                    }
                    axum::extract::ws::Message::Close(_) => break,
                    _ => {}
                }
            }
        })
    };

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }
}

async fn handle_dm_message(
    ws_state: &WsState,
    db_state: &AppState,
    auth_user: &AuthUser,
    msg: WsMessage,
) -> AppResult<()> {
    match msg {
        WsMessage::Connect { user_id, username, channel_type, channel_id } => {
            if user_id != auth_user.id {
                return Err(AppError::Unauthorized("User ID mismatch".to_string()));
            }

            // For DM: channel_id is the other user's ID
            let (user1, user2) = if user_id < channel_id {
                (user_id, channel_id)
            } else {
                (channel_id, user_id)
            };
            let channel_key = format!("dm:{}:{}", user1, user2);

            // Add user to channel
            let user = ConnectedUser { user_id, username: username.clone(), channel_type, channel_id };
            let (tx, rx) = mpsc::unbounded_channel();
            ws_state.add_user_to_channel(channel_key.clone(), user, tx.clone()).await;

            // Send confirmation
            let resp = serde_json::to_string(&WsMessage::Connected {
                user_id,
                message: "Connected to DM".to_string(),
            })?;
            tx.send(resp)?;

            // Send message history
            load_dm_history(db_state, user1, user2, &tx).await?;
        }

        WsMessage::Message { body, channel_type, channel_id, client_nonce } => {
            if body.trim().is_empty() || body.len() > 2000 {
                return Err(AppError::Validation(
                    "Message must be 1-2000 characters".to_string(),
                ));
            }

            // Save to database
            let (user1, user2) = if auth_user.id < channel_id {
                (auth_user.id, channel_id)
            } else {
                (channel_id, auth_user.id)
            };

            let result = sqlx::query(
                "INSERT INTO direct_messages (sender_id, receiver_id, body, client_nonce, read) VALUES (?, ?, ?, ?, 0)"
            )
            .bind(auth_user.id)
            .bind(channel_id)
            .bind(body.trim())
            .bind(&client_nonce)
            .execute(&db_state.db)
            .await?;

            let message_id = result.last_insert_id();
            let channel_key = format!("dm:{}:{}", user1, user2);

            // Broadcast to both users
            let new_message = WsMessage::NewMessage {
                id: message_id,
                user_id: auth_user.id,
                username: auth_user.username.clone(),
                body: body.clone(),
                channel_type,
                channel_id,
                created_at: Utc::now(),
                is_deleted: false,
            };

            let msg_json = serde_json::to_string(&new_message)?;
            ws_state.broadcast_to_channel(&channel_key, msg_json, None).await;
        }

        WsMessage::Ping => {
            // Respond with pong
        }

        _ => {}
    }

    Ok(())
}

async fn load_dm_history(
    state: &AppState,
    user1: u64,
    user2: u64,
    tx: &mpsc::UnboundedSender<String>,
) -> AppResult<()> {
    let messages: Vec<(u64, u64, String, String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT id, sender_id, (SELECT username FROM users WHERE id = sender_id), body, created_at \
         FROM direct_messages \
         WHERE ((sender_id = ? AND receiver_id = ?) OR (sender_id = ? AND receiver_id = ?)) \
         AND is_deleted = 0 \
         ORDER BY created_at DESC \
         LIMIT 50"
    )
    .bind(user1)
    .bind(user2)
    .bind(user2)
    .bind(user1)
    .fetch_all(&state.db)
    .await?;

    let history: Vec<_> = messages
        .into_iter()
        .map(|(id, user_id, username, body, created_at)| crate::ws::message::HistoryMessage {
            id,
            user_id,
            username,
            body,
            created_at,
            is_deleted: false,
        })
        .collect();

    let history_msg = serde_json::to_string(&WsMessage::History { messages: history })?;
    tx.send(history_msg)?;

    Ok(())
}

// ============================================================================
// Hot Town WebSocket Handler
// ============================================================================

pub async fn hot_town_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(channel_id): Path<u16>,
    user: AuthUser,
) -> impl IntoResponse {
    let ws_state = WsState::new();
    ws.on_upgrade(move |socket| handle_hot_town_socket(socket, state, user, channel_id, ws_state))
}

async fn handle_hot_town_socket(
    socket: WebSocket,
    db_state: AppState,
    auth_user: AuthUser,
    channel_id: u16,
    ws_state: WsState,
) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel();

    // Verify user has access to this channel
    let has_access = verify_hot_town_access(&db_state, channel_id, auth_user.college_id).await;
    if !has_access {
        let _ = sender.send(
            axum::extract::ws::Message::Text(
                serde_json::to_string(&WsMessage::Error {
                    message: "Access denied".to_string(),
                }).unwrap()
            )
        ).await;
        return;
    }

    let mut send_task = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if sender.send(axum::extract::ws::Message::Text(message)).await.is_err() {
                break;
            }
        }
    });

    let mut recv_task = {
        let ws_state = ws_state.clone();
        let auth_user = auth_user.clone();
        
        tokio::spawn(async move {
            while let Some(Ok(msg)) = receiver.next().await {
                match msg {
                    axum::extract::ws::Message::Text(text) => {
                        if let Ok(ws_msg) = from_str::<WsMessage>(&text) {
                            if let Err(e) = handle_hot_town_message(&ws_state, &db_state, &auth_user, channel_id, ws_msg).await {
                                let error_msg = serde_json::to_string(&WsMessage::Error {
                                    message: e.to_string(),
                                }).unwrap();
                                let _ = tx.send(error_msg);
                            }
                        }
                    }
                    axum::extract::ws::Message::Close(_) => break,
                    _ => {}
                }
            }
        })
    };

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }
}

async fn handle_hot_town_message(
    ws_state: &WsState,
    db_state: &AppState,
    auth_user: &AuthUser,
    channel_id: u16,
    msg: WsMessage,
) -> AppResult<()> {
    match msg {
        WsMessage::Connect { user_id, username, .. } => {
            if user_id != auth_user.id {
                return Err(AppError::Unauthorized("User ID mismatch".to_string()));
            }

            let channel_key = format!("hot_town:{}", channel_id);
            let user = ConnectedUser {
                user_id,
                username: username.clone(),
                channel_type: "hot_town".to_string(),
                channel_id: channel_id as u64,
            };
            
            let (tx, _rx) = mpsc::unbounded_channel();
            ws_state.add_user_to_channel(channel_key.clone(), user, tx.clone()).await;

            let resp = serde_json::to_string(&WsMessage::Connected {
                user_id,
                message: "Connected to Hot Town".to_string(),
            })?;
            tx.send(resp)?;

            load_hot_town_history(db_state, channel_id, &tx).await?;
        }

        WsMessage::Message { body, .. } => {
            if body.trim().is_empty() || body.len() > 2000 {
                return Err(AppError::Validation(
                    "Message must be 1-2000 characters".to_string(),
                ));
            }

            let result = sqlx::query(
                "INSERT INTO messages (channel_id, user_id, body) VALUES (?, ?, ?)"
            )
            .bind(channel_id)
            .bind(auth_user.id)
            .bind(body.trim())
            .execute(&db_state.db)
            .await?;

            let message_id = result.last_insert_id();
            let channel_key = format!("hot_town:{}", channel_id);

            let new_message = WsMessage::NewMessage {
                id: message_id,
                user_id: auth_user.id,
                username: auth_user.username.clone(),
                body: body.clone(),
                channel_type: "hot_town".to_string(),
                channel_id: channel_id as u64,
                created_at: Utc::now(),
                is_deleted: false,
            };

            let msg_json = serde_json::to_string(&new_message)?;
            ws_state.broadcast_to_channel(&channel_key, msg_json, None).await;
        }

        _ => {}
    }

    Ok(())
}

async fn load_hot_town_history(
    state: &AppState,
    channel_id: u16,
    tx: &mpsc::UnboundedSender<String>,
) -> AppResult<()> {
    let messages: Vec<(u64, u64, String, String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT m.id, m.user_id, u.username, m.body, m.created_at \
         FROM messages m \
         JOIN users u ON u.id = m.user_id \
         WHERE m.channel_id = ? AND m.is_deleted = 0 \
         ORDER BY m.created_at DESC \
         LIMIT 50"
    )
    .bind(channel_id)
    .fetch_all(&state.db)
    .await?;

    let history: Vec<_> = messages
        .into_iter()
        .map(|(id, user_id, username, body, created_at)| crate::ws::message::HistoryMessage {
            id,
            user_id,
            username,
            body,
            created_at,
            is_deleted: false,
        })
        .collect();

    let history_msg = serde_json::to_string(&WsMessage::History { messages: history })?;
    tx.send(history_msg)?;

    Ok(())
}

async fn verify_hot_town_access(
    state: &AppState,
    channel_id: u16,
    college_id: u8,
) -> bool {
    let result: Option<(u16,)> = sqlx::query_as(
        "SELECT ch.id FROM hot_town_channels ch \
         JOIN hot_town_servers s ON s.id = ch.server_id \
         WHERE ch.id = ? AND s.college_id = ?"
    )
    .bind(channel_id)
    .bind(college_id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    result.is_some()
}
