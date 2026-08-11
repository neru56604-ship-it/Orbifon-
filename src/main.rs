mod auth;
mod config;
mod db;
mod error;
mod feed;
mod hot_town;
mod models;
mod routes;

use sqlx::MySqlPool;

use config::Config;

/// Shared application state, cloned cheaply into every handler via
/// Axum's `State` extractor (MySqlPool and Config are both internally
/// Arc-backed / cheap to clone). Axum's blanket `impl FromRef<T> for T`
/// is what lets `AuthUser`'s extractor (which requires
/// `AppState: FromRef<S>`) work directly against this type.
#[derive(Clone)]
pub struct AppState {
    pub db: MySqlPool,
    pub config: Config,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let config = Config::from_env();
    tracing::info!("Starting Orbifon (Gwalior pilot) on port {}", config.port);

    let pool = db::create_pool(&config.database_url).await;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run database migrations");

    let state = AppState {
        db: pool,
        config: config.clone(),
    };

    let app = routes::build_router(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", config.port))
        .await
        .expect("Failed to bind port");

    tracing::info!("Orbifon listening on http://0.0.0.0:{}", config.port);
    axum::serve(listener, app)
        .await
        .expect("Server crashed");
}
