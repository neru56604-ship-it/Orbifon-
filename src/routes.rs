use axum::{
    routing::{get, post},
    Router,
};
use tower_http::cors::{Any, CorsLayer};

use crate::{auth, feed, hot_town, AppState};

pub fn build_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any) // tighten to the real frontend origin(s) before prod launch
        .allow_methods(Any)
        .allow_headers(Any);

    let auth_routes = Router::new()
        .route("/api/auth/register", post(auth::register))
        .route("/api/auth/login", post(auth::login));

    let feed_routes = Router::new()
        .route("/api/feed", get(feed::get_feed))
        .route("/api/posts", post(feed::create_post))
        .route("/api/posts/:post_id/vote", post(feed::vote_post))
        .route(
            "/api/posts/:post_id/comments",
            get(feed::get_comments).post(feed::create_comment),
        )
        .route("/api/posts/:post_id/repost", post(feed::repost))
        .route("/api/uploads/image", post(feed::upload_image));

    let hot_town_routes = Router::new()
        .route("/api/hot-town/my-server", get(hot_town::get_my_server))
        .route(
            "/api/hot-town/channels/:channel_id/messages",
            get(hot_town::get_messages).post(hot_town::post_message),
        );

    Router::new()
        .merge(auth_routes)
        .merge(feed_routes)
        .merge(hot_town_routes)
        .route("/api/health", get(|| async { "orbifon-ok" }))
        .layer(cors)
        .with_state(state)
}
