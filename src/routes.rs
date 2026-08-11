use axum::routing::{delete, get, patch, post};
use tower_http::trace::TraceLayer;

pub fn build_router(state: crate::AppState) -> axum::Router {
    axum::Router::new()
        // Auth routes
        .route("/api/auth/register", post(crate::auth::register))
        .route("/api/auth/login", post(crate::auth::login))

        // Feed routes
        .route("/api/feed", get(crate::feed::get_feed))
        .route("/api/posts", post(crate::feed::create_post))
        .route("/api/uploads/image", post(crate::feed::upload_image))
        .route("/api/posts/:id/vote", post(crate::feed::vote_post))
        .route(
            "/api/posts/:id/comments",
            get(crate::feed::get_comments).post(crate::feed::create_comment),
        )
        .route("/api/posts/:id/repost", post(crate::feed::repost))

        // Profile routes
        .route("/api/users/:username", get(crate::profiles::get_user_profile))
        .route("/api/users/me", patch(crate::profiles::update_user_profile))
        .route("/api/users/:username/followers", get(crate::profiles::get_user_followers))
        .route("/api/users/:username/following", get(crate::profiles::get_user_following))

        // Follow routes
        .route("/api/users/:user_id/follow", post(crate::follows::follow_user))
        .route("/api/users/:user_id/unfollow", delete(crate::follows::unfollow_user))
        .route("/api/users/:user_id/is-following", get(crate::follows::is_following))

        // Search routes
        .route("/api/search", get(crate::search::universal_search))
        .route("/api/search/posts", get(crate::search::search_posts))
        .route("/api/search/users", get(crate::search::search_users))
        .route("/api/search/hashtags", get(crate::search::search_hashtags))

        // Hashtag routes
        .route("/api/hashtags/:tag", get(crate::hashtags::get_posts_by_hashtag))
        .route("/api/hashtags/trending", get(crate::hashtags::get_trending_hashtags))

        // Bookmark routes
        .route("/api/posts/:id/bookmark", post(crate::bookmarks::bookmark_post))
        .route("/api/posts/:id/unbookmark", delete(crate::bookmarks::unbookmark_post))
        .route("/api/bookmarks", get(crate::bookmarks::get_user_bookmarks))

        // Hot Town routes
        .route("/api/hot-town/my-server", get(crate::hot_town::get_my_server))
        .route(
            "/api/hot-town/channels/:id/messages",
            get(crate::hot_town::get_messages).post(crate::hot_town::create_message),
        )

        // WebSocket routes
        .route("/api/ws/dm", get(crate::ws::dm_ws_handler))
        .route("/api/ws/hot-town/:channel_id", get(crate::ws::hot_town_ws_handler))

        // Moderation routes
        .route("/api/reports", post(crate::moderation::create_report))
        .route("/api/mod/reports/pending", get(crate::moderation::list_pending_reports))
        .route("/api/mod/reports/:id/review", patch(crate::moderation::review_report))

        // Health & Stats
        .route("/api/health", get(|| async { "OK" }))
        .route("/api/stats", get(get_server_stats))

        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn get_server_stats(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
) -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "healthy",
        "database": "connected",
        "cache": if state.message_cache.is_some() { "redis" } else { "in-memory" },
        "version": "0.3.0"
    }))
}
