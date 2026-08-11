use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    models::*,
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

// =====================================================================
// MODERATION MODELS
// =====================================================================

#[derive(Debug, sqlx::FromRow)]
pub struct ReportRow {
    pub id: u64,
    pub reporter_id: u64,
    pub reported_user_id: Option<u64>,
    pub post_id: Option<u64>,
    pub comment_id: Option<u64>,
    pub message_id: Option<u64>,
    pub report_type: String,
    pub reason: String,
    pub status: String,
    pub action_taken: Option<String>,
    pub moderator_id: Option<u64>,
    pub reviewed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct ReportResponse {
    pub id: u64,
    pub reporter_username: String,
    pub reported_content_type: String,
    pub report_type: String,
    pub reason: String,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateReportRequest {
    pub report_type: String,
    pub reason: String,
    pub post_id: Option<u64>,
    pub comment_id: Option<u64>,
    pub message_id: Option<u64>,
    pub reported_user_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct ReviewReportRequest {
    pub status: String,
    pub action_taken: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct UserSuspensionRow {
    pub id: u64,
    pub user_id: u64,
    pub reason: String,
    pub suspended_until: Option<chrono::DateTime<chrono::Utc>>,
    pub created_by: u64,
    pub is_permanent: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ReportsQuery {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub sort: Option<String>,
    pub report_type: Option<String>,
}

// =====================================================================
// REPORT CREATION (User-facing)
// =====================================================================

pub async fn create_report(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<CreateReportRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    // Validate report type
    let valid_types = vec!["spam", "harassment", "nsfw", "violence", "misinformation"];
    if !valid_types.contains(&req.report_type.as_str()) {
        return Err(AppError::Validation(
            "Invalid report type".to_string(),
        ));
    }

    // Validate reason length
    if req.reason.trim().is_empty() || req.reason.len() > 500 {
        return Err(AppError::Validation(
            "Reason must be 1-500 characters".to_string(),
        ));
    }

    // Ensure at least one content type is reported
    let content_count = [req.post_id, req.comment_id, req.message_id]
        .iter()
        .filter(|x| x.is_some())
        .count();

    if content_count == 0 && req.reported_user_id.is_none() {
        return Err(AppError::Validation(
            "Must report either content or a user".to_string(),
        ));
    }

    // Prevent self-reporting
    if let Some(reported_user_id) = req.reported_user_id {
        if reported_user_id == user.id {
            return Err(AppError::Validation(
                "You cannot report yourself".to_string(),
            ));
        }
    }

    // Check for duplicate recent reports (rate limiting)
    let recent_report: Option<(u64,)> = sqlx::query_as(
        "SELECT id FROM reports WHERE reporter_id = ? AND created_at > DATE_SUB(NOW(), INTERVAL 1 HOUR) LIMIT 1"
    )
    .bind(user.id)
    .fetch_optional(&state.db)
    .await?;

    if recent_report.is_some() {
        return Err(AppError::Validation(
            "You can only submit one report per hour".to_string(),
        ));
    }

    let result = sqlx::query(
        "INSERT INTO reports (reporter_id, reported_user_id, post_id, comment_id, message_id, report_type, reason, status) \
         VALUES (?, ?, ?, ?, ?, ?, ?, 'pending')"
    )
    .bind(user.id)
    .bind(req.reported_user_id)
    .bind(req.post_id)
    .bind(req.comment_id)
    .bind(req.message_id)
    .bind(&req.report_type)
    .bind(&req.reason)
    .execute(&state.db)
    .await?;

    // Log for audit
    tracing::warn!(
        report_id = result.last_insert_id(),
        reporter_id = user.id,
        report_type = req.report_type,
        "New moderation report submitted"
    );

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "report_id": result.last_insert_id(),
            "message": "Report submitted. Thank you for helping keep Orbifon safe."
        })),
    ))
}

// =====================================================================
// MODERATION DASHBOARD (Moderator-only)
// =====================================================================

pub async fn list_pending_reports(
    State(state): State<AppState>,
    moderator: AuthUser,
    Query(q): Query<ReportsQuery>,
) -> AppResult<Json<Vec<ReportResponse>>> {
    // Permission check
    check_moderator_role(&state, moderator.id).await?;

    let limit = q.limit.unwrap_or(25).min(100);
    let offset = q.offset.unwrap_or(0);
    let sort_by = match q.sort.as_deref() {
        Some("oldest") => "created_at ASC",
        _ => "created_at DESC",
    };

    let filter_type = match &q.report_type {
        Some(rt) if !rt.is_empty() => format!(" AND report_type = '{}'", 
            rt.replace("'", "")),
        _ => String::new(),
    };

    let sql = format!(
        "SELECT * FROM reports WHERE status = 'pending' {} ORDER BY {} LIMIT ? OFFSET ?",
        filter_type, sort_by
    );

    let rows: Vec<ReportRow> = sqlx::query_as(&sql)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await?;

    let responses = rows.into_iter().map(|r| {
        ReportResponse {
            id: r.id,
            reporter_username: format!("User {}", r.reporter_id),
            reported_content_type: if r.post_id.is_some() { "post".to_string() }
                else if r.comment_id.is_some() { "comment".to_string() }
                else if r.message_id.is_some() { "message".to_string() }
                else { "user".to_string() },
            report_type: r.report_type,
            reason: r.reason,
            status: r.status,
            created_at: r.created_at,
        }
    }).collect();

    Ok(Json(responses))
}

pub async fn review_report(
    State(state): State<AppState>,
    moderator: AuthUser,
    Path(report_id): Path<u64>,
    Json(req): Json<ReviewReportRequest>,
) -> AppResult<Json<serde_json::Value>> {
    check_moderator_role(&state, moderator.id).await?;

    if !["dismissed", "actioned"].contains(&req.status.as_str()) {
        return Err(AppError::Validation("Invalid status".to_string()));
    }

    let mut tx = state.db.begin().await?;

    // Fetch the report
    let report: ReportRow = sqlx::query_as(
        "SELECT * FROM reports WHERE id = ? FOR UPDATE"
    )
    .bind(report_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Report not found".to_string()))?;

    // Execute action if actioned
    if req.status == "actioned" {
        if let Some(action) = &req.action_taken {
            match action.as_str() {
                "content_removed" => {
                    // Remove the reported content
                    if let Some(post_id) = report.post_id {
                        sqlx::query("UPDATE posts SET is_deleted = 1 WHERE id = ?")
                            .bind(post_id)
                            .execute(&mut *tx)
                            .await?;
                    } else if let Some(comment_id) = report.comment_id {
                        sqlx::query("UPDATE comments SET is_deleted = 1 WHERE id = ?")
                            .bind(comment_id)
                            .execute(&mut *tx)
                            .await?;
                    } else if let Some(message_id) = report.message_id {
                        sqlx::query("UPDATE messages SET is_deleted = 1 WHERE id = ?")
                            .bind(message_id)
                            .execute(&mut *tx)
                            .await?;
                    }
                }
                "user_suspended" => {
                    if let Some(reported_user_id) = report.reported_user_id {
                        // Suspend user for 7 days
                        let suspended_until = chrono::Utc::now() + chrono::Duration::days(7);
                        sqlx::query(
                            "INSERT INTO user_suspensions (user_id, reason, suspended_until, created_by, is_permanent) VALUES (?, ?, ?, ?, 0)"
                        )
                        .bind(reported_user_id)
                        .bind(format!("Suspended: {}", report.report_type))
                        .bind(suspended_until)
                        .bind(moderator.id)
                        .execute(&mut *tx)
                        .await?;

                        sqlx::query("UPDATE users SET is_active = 0 WHERE id = ?")
                            .bind(reported_user_id)
                            .execute(&mut *tx)
                            .await?;
                    }
                }
                "warning" => {
                    // Send warning notification (implement later)
                    tracing::info!("Warning issued to user");
                }
                _ => {}
            }
        }
    }

    // Update report status
    sqlx::query(
        "UPDATE reports SET status = ?, action_taken = ?, moderator_id = ?, reviewed_at = NOW() WHERE id = ?"
    )
    .bind(&req.status)
    .bind(&req.action_taken)
    .bind(moderator.id)
    .bind(report_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    tracing::info!(
        report_id = report_id,
        moderator_id = moderator.id,
        action = ?req.action_taken,
        "Report reviewed by moderator"
    );

    Ok(Json(serde_json::json!({
        "report_id": report_id,
        "status": req.status,
        "action_taken": req.action_taken
    })))
}

// =====================================================================
// UTILITIES
// =====================================================================

async fn check_moderator_role(state: &AppState, user_id: u64) -> AppResult<()> {
    let is_moderator: Option<(bool,)> = sqlx::query_as(
        "SELECT is_moderator FROM users WHERE id = ? AND is_active = 1"
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;

    match is_moderator {
        Some((true,)) => Ok(()),
        _ => Err(AppError::Forbidden(
            "Moderator access required".to_string(),
        )),
    }
}
