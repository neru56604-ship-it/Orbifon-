use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use crate::ws::message::ConnectedUser;

#[derive(Clone)]
pub struct WsState {
    // Key: "dm:{user1_id}:{user2_id}" or "hot_town:{channel_id}"
    // Value: Vec of connected users in that channel (on THIS server)
    pub channels: Arc<RwLock<HashMap<String, Vec<ConnectedUser>>>>,
    
    // Key: user_id
    // Value: sender to that user's WebSocket (on THIS server)
    pub user_senders: Arc<RwLock<HashMap<u64, mpsc::UnboundedSender<String>>>>,
    
    // Connection count for monitoring
    pub total_connections: Arc<RwLock<u64>>,
}

impl WsState {
    pub fn new() -> Self {
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            user_senders: Arc::new(RwLock::new(HashMap::new())),
            total_connections: Arc::new(RwLock::new(0)),
        }
    }

    pub async fn add_user_to_channel(
        &self,
        channel_key: String,
        user: ConnectedUser,
        sender: mpsc::UnboundedSender<String>,
    ) {
        let mut channels = self.channels.write().await;
        channels.entry(channel_key).or_insert_with(Vec::new).push(user.clone());
        
        let mut senders = self.user_senders.write().await;
        senders.insert(user.user_id, sender);
        
        let mut total = self.total_connections.write().await;
        *total += 1;
        
        tracing::info!(
            "User connected: {} (total: {})",
            user.user_id,
            *total
        );
    }

    pub async fn remove_user_from_channel(
        &self,
        channel_key: &str,
        user_id: u64,
    ) {
        let mut channels = self.channels.write().await;
        if let Some(users) = channels.get_mut(channel_key) {
            users.retain(|u| u.user_id != user_id);
        }
        
        let mut senders = self.user_senders.write().await;
        senders.remove(&user_id);
        
        let mut total = self.total_connections.write().await;
        if *total > 0 {
            *total -= 1;
        }
        
        tracing::info!(
            "User disconnected: {} (total: {})",
            user_id,
            *total
        );
    }

    pub async fn get_channel_users(&self, channel_key: &str) -> Vec<ConnectedUser> {
        let channels = self.channels.read().await;
        channels
            .get(channel_key)
            .map(|users| users.clone())
            .unwrap_or_default()
    }

    pub async fn broadcast_to_channel(
        &self,
        channel_key: &str,
        message: String,
        exclude_user_id: Option<u64>,
    ) {
        let users = self.get_channel_users(channel_key).await;
        let senders = self.user_senders.read().await;
        
        for user in users {
            if let Some(exclude_id) = exclude_user_id {
                if user.user_id == exclude_id {
                    continue;
                }
            }
            
            if let Some(sender) = senders.get(&user.user_id) {
                let _ = sender.send(message.clone());
            }
        }
    }

    pub async fn send_to_user(&self, user_id: u64, message: String) -> bool {
        let senders = self.user_senders.read().await;
        if let Some(sender) = senders.get(&user_id) {
            sender.send(message).is_ok()
        } else {
            false
        }
    }
    
    pub async fn get_total_connections(&self) -> u64 {
        *self.total_connections.read().await
    }
}
