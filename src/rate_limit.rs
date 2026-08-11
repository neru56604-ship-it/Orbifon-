use crate::{
    error::{AppError, AppResult},
    AppState,
};

pub struct RateLimiter;

impl RateLimiter {
    /// Check if user exceeded rate limit for action
    pub async fn check_action(
        state: &AppState,
        user_id: u64,
        action: &str,
        limit: u32,
        window_seconds: u64,
    ) -> AppResult<()> {
        let count: (u32,) = sqlx::query_as(
            "SELECT COUNT(*) FROM rate_limit_events WHERE user_id = ? AND action = ? AND created_at > DATE_SUB(NOW(), INTERVAL ? SECOND)"
        )
        .bind(user_id)
        .bind(action)
        .bind(window_seconds as i32)
        .fetch_one(&state.db)
        .await?;

        if count.0 >= limit {
            return Err(AppError::TooManyRequests(
                format!("Rate limit exceeded for: {}", action)
            ));
        }

        // Log this action
        let _ = sqlx::query(
            "INSERT INTO rate_limit_events (user_id, action) VALUES (?, ?)"
        )
        .bind(user_id)
        .bind(action)
        .execute(&state.db)
        .await;

        Ok(())
    }
}
