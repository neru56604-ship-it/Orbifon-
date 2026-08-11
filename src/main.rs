mod audit_log;
mod auth;
mod bookmarks;
mod cache;
mod config;
mod db;
mod error;
mod feed;
mod follows;
mod hashtags;
mod hot_town;
mod moderation;
mod models;
mod profiles;
mod rate_limit;
mod reputation;
mod routes;
mod search;
mod tags;
mod ws;

use sqlx::MySqlPool;
use cache::{MessageCache, PubSubManager, ConnectionLimiter, RedisPool};
use config::Config;

#[derive(Clone)]
pub struct AppState {
    pub db: MySqlPool,
    pub config: Config,
    pub message_cache: Option<MessageCache>,
    pub pubsub_manager: Option<PubSubManager>,
    pub connection_limiter: Option<ConnectionLimiter>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let config = Config::from_env();
    tracing::info!("Starting Orbifon v0.3.0 on port {}", config.port);

    let pool = db::create_pool(&config.database_url).await;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run database migrations");

    let (message_cache, pubsub_manager, connection_limiter) = 
        if let Ok(redis_url) = std::env::var("REDIS_URL") {
            match RedisPool::new(&redis_url).await {
                Ok(redis) => {
                    tracing::info!("✅ Redis connected for caching and Pub/Sub");
                    (
                        Some(MessageCache::new(redis.clone())),
                        Some(PubSubManager::new(redis.clone())),
                        Some(ConnectionLimiter::new(redis)),
                    )
                }
                Err(e) => {
                    tracing::warn!("⚠️  Redis connection failed: {}. Single-server mode.", e);
                    (None, None, None)
                }
            }
        } else {
            tracing::warn!("⚠️  REDIS_URL not set. Single-server mode.");
            (None, None, None)
        };

    let state = AppState {
        db: pool,
        config: config.clone(),
        message_cache,
        pubsub_manager,
        connection_limiter,
    };

    let app = routes::build_router(state);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", config.port))
        .await
        .expect("Failed to bind port");

    tracing::info!("🚀 Orbifon listening on http://0.0.0.0:{}", config.port);
    axum::serve(listener, app).await.expect("Server crashed");
}
