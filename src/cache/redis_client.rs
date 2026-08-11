use redis::aio::ConnectionManager;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct RedisPool {
    pub manager: ConnectionManager,
}

impl RedisPool {
    pub async fn new(redis_url: &str) -> Result<Self, redis::RedisError> {
        let client = redis::Client::open(redis_url)?;
        let manager = ConnectionManager::new(client).await?;
        Ok(Self { manager })
    }
}

// ============================================================================
// Message Cache (Redis)
// ============================================================================

pub struct MessageCache {
    redis: RedisPool,
}

impl MessageCache {
    pub fn new(redis: RedisPool) -> Self {
        Self { redis }
    }

    /// Store message in Redis (fast, short-lived)
    pub async fn cache_message(
        &self,
        key: &str,
        message: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = self.redis.manager.clone();
        
        // LPUSH (left push to list - newest first)
        redis::cmd("LPUSH")
            .arg(key)
            .arg(&message)
            .query_async(&mut conn)
            .await?;
        
        // LTRIM (keep only last 1000 messages)
        redis::cmd("LTRIM")
            .arg(key)
            .arg(0)
            .arg(999)
            .query_async(&mut conn)
            .await?;
        
        // EXPIRE (delete after 24 hours)
        redis::cmd("EXPIRE")
            .arg(key)
            .arg(86400)
            .query_async(&mut conn)
            .await?;

        Ok(())
    }

    /// Get messages from Redis cache
    pub async fn get_cached_messages(
        &self,
        key: &str,
        limit: usize,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let mut conn = self.redis.manager.clone();
        
        // LRANGE (get range)
        let messages: Vec<String> = redis::cmd("LRANGE")
            .arg(key)
            .arg(0)
            .arg(limit - 1)
            .query_async(&mut conn)
            .await?;

        Ok(messages)
    }

    /// Store offline message (for when user is disconnected)
    pub async fn queue_offline_message(
        &self,
        user_id: u64,
        message: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = self.redis.manager.clone();
        let key = format!("offline_messages:{}", user_id);
        
        redis::cmd("LPUSH")
            .arg(&key)
            .arg(&message)
            .query_async(&mut conn)
            .await?;
        
        // Keep for 7 days
        redis::cmd("EXPIRE")
            .arg(&key)
            .arg(604800)
            .query_async(&mut conn)
            .await?;

        Ok(())
    }

    /// Get all offline messages for a user
    pub async fn get_offline_messages(
        &self,
        user_id: u64,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let mut conn = self.redis.manager.clone();
        let key = format!("offline_messages:{}", user_id);
        
        let messages: Vec<String> = redis::cmd("LRANGE")
            .arg(&key)
            .arg(0)
            .arg(-1)
            .query_async(&mut conn)
            .await?;
        
        // Delete after fetching
        redis::cmd("DEL")
            .arg(&key)
            .query_async(&mut conn)
            .await?;

        Ok(messages)
    }
}

// ============================================================================
// Pub/Sub for Cross-Server Broadcasting
// ============================================================================

pub struct PubSubManager {
    redis: RedisPool,
}

impl PubSubManager {
    pub fn new(redis: RedisPool) -> Self {
        Self { redis }
    }

    /// Publish message to all servers subscribed to this channel
    pub async fn publish(
        &self,
        channel: &str,
        message: String,
    ) -> Result<u64, Box<dyn std::error::Error>> {
        let mut conn = self.redis.manager.clone();
        
        let num_subscribers: u64 = redis::cmd("PUBLISH")
            .arg(channel)
            .arg(&message)
            .query_async(&mut conn)
            .await?;

        Ok(num_subscribers)
    }

    /// Subscribe to channel (for listening to broadcasts from other servers)
    pub async fn subscribe(
        &self,
        channels: &[&str],
    ) -> Result<redis::aio::PubSub, redis::RedisError> {
        let client = redis::Client::open(&self.redis.manager.connection_info().addr.to_string())?;
        let pubsub = client.get_async_connection().await?.into_pubsub();
        Ok(pubsub)
    }
}

// ============================================================================
// Connection Pool (Rate Limiting)
// ============================================================================

pub struct ConnectionLimiter {
    redis: RedisPool,
}

impl ConnectionLimiter {
    pub fn new(redis: RedisPool) -> Self {
        Self { redis }
    }

    /// Check if user is rate limited
    pub async fn check_rate_limit(
        &self,
        user_id: u64,
        action: &str,
        limit: u32,
        window_seconds: u64,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let mut conn = self.redis.manager.clone();
        let key = format!("ratelimit:{}:{}", user_id, action);
        
        let count: u32 = redis::cmd("INCR")
            .arg(&key)
            .query_async(&mut conn)
            .await?;
        
        if count == 1 {
            redis::cmd("EXPIRE")
                .arg(&key)
                .arg(window_seconds)
                .query_async(&mut conn)
                .await?;
        }
        
        Ok(count >= limit)
    }
}
