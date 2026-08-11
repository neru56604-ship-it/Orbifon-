use axum::{
    extract::{ws::{WebSocket, WebSocketUpgrade}, State, Path},
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use serde_json::{json, from_str};
use tokio::sync::mpsc;
use chrono::Utc;
use std::time::Duration;

use crate::{
    AppState, auth::AuthUser, error::{AppError, AppResult}, 
    ws::{WsMessage, WsState, ConnectedUser},
    cache::{MessageCache, PubSubManager, ConnectionLimiter},
};

// ============================================================================
// Constants for Scaling
// ============================================================================

const MESSAGE_HISTORY_LIMIT: usize = 50;
const MAX_MESSAGE_SIZE: usize = 2000;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes
const PING_INTERVAL: Duration = Duration::from_secs(30);

// ============================================================================
// DM WebSocket Handler (Optimized for Scale)
// ============================================================================

pub async fn dm_ws_handler(
    ws: WebSocketUpgrade,
    State(db_state): State<AppState>,
    user: AuthUser,
) -> impl IntoResponse {
    let ws_state = WsState::new();
    
    ws.on_upgrade(move |socket| {
        handle_dm_socket(socket, db_state, user, ws_state)
    })
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
        let mut ping_interval = tokio::time::interval(PING_INTERVAL);
        
        loop {
            tokio::select! {
                Some(message) = rx.recv() => {
                    if sender.send(axum::extract::ws::Message::Text(message)).await.is_err() {
                        break;
                    }
                }
                _ = ping_interval.tick() => {
                    if let Ok(pong) = serde_json::to_string(&WsMessage::Pong) {
                        if sender.send(axum::extract::ws::Message::Text(pong)).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    });

    let auth_user_clone = auth_user.clone();
    let ws_state_clone = ws_state.clone();
    let db_state_clone = db_state.clone();
    
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                axum::extract::ws::Message::Text(text) => {
                    if let Ok(ws_msg) = from_str::<WsMessage>(&text) {
                        if let Err(e) = handle_dm_message(
                            &ws_state_clone,
                            &db_state_clone,
                            &auth_user_clone,
                            ws_msg,
                        )
                        .await
                        {
                            let error_msg = serde_json::to_string(&WsMessage::Error {
                                message: e.to_string(),
                            })
                            .unwrap();
                            let _ = tx.send(error_msg);
                        }
                    }
                }
                axum::extract::ws::Message::Close(_) => {
                    let channel_key = format!("dm:{}:{}", 
                        std::cmp::min(auth_user_clone.id, 0),
                        std::cmp::max(auth_user_clone.id, 0)
                    );
                    ws_state_clone.remove_user_from_channel(&channel_key, auth_user_clone.id).await;
                    break;
                }
                _ => {}
            }
        }
    });

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

            // Check rate limit
            if let Some(limiter) = &db_state.connection_limiter {
                if limiter.check_rate_limit(user_id, "dm_connect", 100, 3600).await.unwrap_or(true) {
                    return Err(AppError::TooManyRequests("Too many connections".to_string()));
                }
            }

            let (user1, user2) = if user_id < channel_id {
                (user_id, channel_id)
            } else {
                (channel_id, user_id)
            };
            let channel_key = format!("dm:{}:{}", user1, user2);

            let user = ConnectedUser { user_id, username: username.clone(), channel_type, channel_id };
            let (tx, _rx) = mpsc::unbounded_channel();
            ws_state.add_user_to_channel(channel_key.clone(), user, tx.clone()).await;

            let resp = serde_json::to_string(&WsMessage::Connected {
                user_id,
                message: "Connected to DM".to_string(),
            })?;
            tx.send(resp)?;

            // Load from Redis cache first (fast)
            if let Some(cache) = &db_state.message_cache {
                let cached = cache.get_cached_messages(&channel_key, MESSAGE_HISTORY_LIMIT).await.unwrap_or_default();
                for msg in cached {
                    let _ = tx.send(msg);
                }
            }
        }

        WsMessage::Message { body, channel_type, channel_id, client_nonce } => {
            if body.trim().is_empty() || body.len() > MAX_MESSAGE_SIZE {
                return Err(AppError::Validation(
                    format!("Message must be 1-{} characters", MAX_MESSAGE_SIZE),
                ));
            }

            // Rate limit
            if let Some(limiter) = &db_state.connection_limiter {
                if limiter.check_rate_limit(auth_user.id, "dm_message", 100, 3600).await.unwrap_or(true) {
                    return Err(AppError::TooManyRequests("Message limit exceeded".to_string()));
                }
            }

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
            
            // Cache in Redis (fast)
            if let Some(cache) = &db_state.message_cache {
                let _ = cache.cache_message(&channel_key, msg_json.clone()).await;
            }
            
            // Broadcast to local connections
            ws_state.broadcast_to_channel(&channel_key, msg_json.clone(), None).await;
            
            // Publish to other servers (Redis Pub/Sub)
            if let Some(pubsub) = &db_state.pubsub_manager {
                let _ = pubsub.publish(&format!("channel:{}", channel_key), msg_json).await;
            }
            
            // Queue for offline users
            if !ws_state.send_to_user(channel_id, msg_json.clone()).await {
                if let Some(cache) = &db_state.message_cache {
                    let _ = cache.queue_offline_message(channel_id, msg_json).await;
                }
            }
        }

        WsMessage::Ping => {
            // Pong response handled by ping_interval
        }

        _ => {}
    }

    Ok(())
}

// ============================================================================
// Hot Town WebSocket Handler (Optimized for Scale)
// ============================================================================

pub async fn hot_town_ws_handler(
    ws: WebSocketUpgrade,
    State(db_state): State<AppState>,
    Path(channel_id): Path<u16>,
    user: AuthUser,
) -> impl IntoResponse {
    let ws_state = WsState::new();
    
    ws.on_upgrade(move |socket| {
        handle_hot_town_socket(socket, db_state, user, channel_id, ws_state)
    })
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

    // Verify access
    let has_access = verify_hot_town_access(&db_state, channel_id, auth_user.college_id).await;
    if !has_access {
        let _ = sender.send(
            axum::extract::ws::Message::Text(
                serde_json::to_string(&WsMessage::Error {
                    message: "Access denied".to_string(),
                })
                .unwrap(),
            ),
        )
        .await;
        return;
    }

    let mut send_task = tokio::spawn(async move {
        let mut ping_interval = tokio::time::interval(PING_INTERVAL);
        
        loop {
            tokio::select! {
                Some(message) = rx.recv() => {
                    if sender.send(axum::extract::ws::Message::Text(message)).await.is_err() {
                        break;
                    }
                }
                _ = ping_interval.tick() => {
                    if let Ok(pong) = serde_json::to_string(&WsMessage::Pong) {
                        if sender.send(axum::extract::ws::Message::Text(pong)).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    });

    let auth_user_clone = auth_user.clone();
    let ws_state_clone = ws_state.clone();
    let db_state_clone = db_state.clone();
    
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                axum::extract::ws::Message::Text(text) => {
                    if let Ok(ws_msg) = from_str::<WsMessage>(&text) {
                        if let Err(e) = handle_hot_town_message(
                            &ws_state_clone,
                            &db_state_clone,
                            &auth_user_clone,
                            channel_id,
                            ws_msg,
                        )
                        .await
                        {
                            let error_msg = serde_json::to_string(&WsMessage::Error {
                                message: e.to_string(),
                            })
                            .unwrap();
                            let _ = tx.send(error_msg);
                        }
                    }
                }
                axum::extract::ws::Message::Close(_) => {
                    let channel_key = format!("hot_town:{}", channel_id);
                    ws_state_clone.remove_user_from_channel(&channel_key, auth_user_clone.id).await;
                    break;
                }
                _ => {}
            }
        }
    });

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

            // Load from Redis cache
            if let Some(cache) = &db_state.message_cache {
                let cached = cache.get_cached_messages(&channel_key, MESSAGE_HISTORY_LIMIT).await.unwrap_or_default();
                for msg in cached {
                    let _ = tx.send(msg);
                }
            }
        }

        WsMessage::Message { body, .. } => {
            if body.trim().is_empty() || body.len() > MAX_MESSAGE_SIZE {
                return Err(AppError::Validation(
                    format!("Message must be 1-{} characters", MAX_MESSAGE_SIZE),
                ));
            }

            // Rate limit
            if let Some(limiter) = &db_state.connection_limiter {
                if limiter.check_rate_limit(auth_user.id, "hot_town_message", 100, 3600).await.unwrap_or(true) {
                    return Err(AppError::TooManyRequests("Message limit exceeded".to_string()));
                }
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
            
            // Cache in Redis
            if let Some(cache) = &db_state.message_cache {
                let _ = cache.cache_message(&channel_key, msg_json.clone()).await;
            }
            
            // Broadcast to local connections
            ws_state.broadcast_to_channel(&channel_key, msg_json.clone(), None).await;
            
            // Publish to other servers
            if let Some(pubsub) = &db_state.pubsub_manager {
                let _ = pubsub.publish(&format!("channel:{}", channel_key), msg_json).await;
            }
        }

        _ => {}
    }

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
