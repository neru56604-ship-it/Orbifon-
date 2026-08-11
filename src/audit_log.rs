use crate::AppState;
use serde_json::Value;

pub struct AuditLog;

impl AuditLog {
    pub async fn log_action(
        state: &AppState,
        actor_id: u64,
        action: &str,
        target_type: &str,
        target_id: u64,
        details: Option<Value>,
    ) {
        let _ = sqlx::query(
            "INSERT INTO audit_logs (actor_id, action, target_type, target_id, details) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(actor_id)
        .bind(action)
        .bind(target_type)
        .bind(target_id)
        .bind(details)
        .execute(&state.db)
        .await;
    }
}
