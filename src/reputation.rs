use crate::{error::AppResult, AppState};

pub struct ReputationSystem;

impl ReputationSystem {
    /// Calculate user reputation score based on:
    /// - Post upvotes (+5 each)
    /// - Comment upvotes (+2 each)
    /// - Reports against user (-20 each actioned report)
    /// - Account age (bonus for older accounts)
    pub async fn calculate_score(state: &AppState, user_id: u64) -> AppResult<i32> {
        let score: (i64,) = sqlx::query_as(
            "SELECT \
                COALESCE(SUM(CASE WHEN type='post_upvote' THEN 5 \
                               WHEN type='comment_upvote' THEN 2 \
                               WHEN type='report_actioned' THEN -20 \
                               WHEN type='suspension' THEN -50 \
                               ELSE 0 END), 0) as score \
             FROM reputation_events WHERE user_id = ?"
        )
        .bind(user_id)
        .fetch_one(&state.db)
        .await?;

        let days_active: (i64,) = sqlx::query_as(
            "SELECT DATEDIFF(NOW(), created_at) FROM users WHERE id = ?"
        )
        .bind(user_id)
        .fetch_one(&state.db)
        .await?;

        let seniority_bonus = (days_active.0 / 30) as i32; // +1 per month

        Ok(std::cmp::max(0, score.0 as i32 + seniority_bonus))
    }

    /// New users with low reputation need higher scrutiny
    pub async fn should_require_moderation_review(
        state: &AppState,
        user_id: u64,
    ) -> AppResult<bool> {
        let score = Self::calculate_score(state, user_id).await?;
        
        // Auto-review posts from users with <50 reputation
        Ok(score < 50)
    }
}
