use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts, State},
    http::{request::Parts, StatusCode},
    Json,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use once_cell::sync::Lazy;
use rand_core::OsRng;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::{
    error::{AppError, AppResult},
    models::{AuthResponse, CollegeRow, LoginRequest, RegisterRequest, UserRow},
    AppState,
};

// ---------------------------------------------------------------------
// GWALIOR PILOT DOMAIN GUARDRAIL
// Strict allow-list regex — only these two domains can ever register.
// Anything else (gmail.com, yahoo.com, a spoofed lookalike domain, etc.)
// is rejected before any DB write happens.
// ---------------------------------------------------------------------
static ALLOWED_EMAIL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^[a-z0-9._%+\-]+@(iiitm\.ac\.in|mitsgwalior\.in)$").unwrap()
});

fn extract_domain(email: &str) -> AppResult<String> {
    if !ALLOWED_EMAIL_RE.is_match(email) {
        return Err(AppError::Validation(
            "Only official IIITM Gwalior (@iiitm.ac.in) or MITS Gwalior (@mitsgwalior.in) \
             emails are allowed on Orbifon."
                .to_string(),
        ));
    }
    let domain = email
        .rsplit('@')
        .next()
        .ok_or_else(|| AppError::Validation("Malformed email".to_string()))?
        .to_lowercase();
    Ok(domain)
}

// ---------------------------------------------------------------------
// JWT claims
// ---------------------------------------------------------------------
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: u64,      // user id
    college_id: u8,
    username: String,
    exp: usize,
}

/// Extracted from a validated Bearer token and injected into any handler
/// that takes `AuthUser` as a parameter. This is the single choke point
/// through which "who is making this request" flows — every protected
/// handler gets it for free via Axum's extractor system.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: u64,
    pub college_id: u8,
    pub username: String,
}

#[async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);

        let header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| AppError::Unauthorized("Missing Authorization header".to_string()))?;

        let token = header
            .strip_prefix("Bearer ")
            .ok_or_else(|| AppError::Unauthorized("Expected 'Bearer <token>'".to_string()))?;

        let data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(app_state.config.jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|_| AppError::Unauthorized("Invalid or expired token".to_string()))?;

        Ok(AuthUser {
            id: data.claims.sub,
            college_id: data.claims.college_id,
            username: data.claims.username,
        })
    }
}

fn issue_jwt(user: &UserRow, secret: &str) -> AppResult<String> {
    let expiry = chrono::Utc::now() + chrono::Duration::days(30);
    let claims = Claims {
        sub: user.id,
        college_id: user.college_id,
        username: user.username.clone(),
        exp: expiry.timestamp() as usize,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(e.into()))
}

fn hash_password(plain: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(plain.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(anyhow::anyhow!("password hash failed: {e}")))
}

fn verify_password(plain: &str, hash: &str) -> AppResult<bool> {
    let parsed = PasswordHash::new(hash)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("bad stored hash: {e}")))?;
    Ok(Argon2::default()
        .verify_password(plain.as_bytes(), &parsed)
        .is_ok())
}

fn validate_registration(req: &RegisterRequest) -> AppResult<()> {
    if req.username.trim().len() < 3 || req.username.len() > 30 {
        return Err(AppError::Validation(
            "Username must be 3-30 characters".to_string(),
        ));
    }
    if !req
        .username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(AppError::Validation(
            "Username can only contain letters, numbers, and underscores".to_string(),
        ));
    }
    if req.password.len() < 8 {
        return Err(AppError::Validation(
            "Password must be at least 8 characters".to_string(),
        ));
    }
    if req.display_name.trim().is_empty() || req.display_name.len() > 60 {
        return Err(AppError::Validation(
            "Display name must be 1-60 characters".to_string(),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    validate_registration(&req)?;
    let domain = extract_domain(&req.email)?;

    let college = sqlx::query_as::<_, CollegeRow>(
        "SELECT id, name, short_tag, email_domain, slug FROM colleges \
         WHERE email_domain = ? AND is_active = 1",
    )
    .bind(&domain)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::Validation("This college domain is not part of the pilot".to_string()))?;

    // Uniqueness pre-checks (the DB's UNIQUE constraints are the real
    // guard; these just give friendlier error messages).
    let existing: Option<(u64,)> =
        sqlx::query_as("SELECT id FROM users WHERE email = ? OR username = ?")
            .bind(&req.email)
            .bind(&req.username)
            .fetch_optional(&state.db)
            .await?;
    if existing.is_some() {
        return Err(AppError::Conflict(
            "Username or email already registered".to_string(),
        ));
    }

    let password_hash = hash_password(&req.password)?;
    let verification_token = uuid::Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO users (college_id, username, email, password_hash, display_name, verification_token) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(college.id)
    .bind(&req.username)
    .bind(&req.email)
    .bind(&password_hash)
    .bind(&req.display_name)
    .bind(&verification_token)
    .execute(&state.db)
    .await?;

    // TODO (Step 10 hardening pass): wire this into a real transactional
    // email service. For pilot purposes we log it so you can verify the
    // flow manually.
    tracing::info!(
        "Verification email for {} — token: {} (college: {})",
        req.email,
        verification_token,
        college.short_tag
    );

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "message": format!(
                "Registered for {}. Check your college email to verify your account.",
                college.short_tag
            )
        })),
    ))
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> AppResult<Json<AuthResponse>> {
    let user = sqlx::query_as::<_, UserRow>(
        "SELECT id, college_id, username, email, password_hash, display_name, avatar_url, is_active \
         FROM users WHERE email = ?",
    )
    .bind(&req.email)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::Unauthorized("Invalid email or password".to_string()))?;

    if !user.is_active {
        return Err(AppError::Forbidden(
            "This account has been suspended".to_string(),
        ));
    }

    if !verify_password(&req.password, &user.password_hash)? {
        return Err(AppError::Unauthorized("Invalid email or password".to_string()));
    }

    let college: CollegeRow = sqlx::query_as(
        "SELECT id, name, short_tag, email_domain, slug FROM colleges WHERE id = ?",
    )
    .bind(user.college_id)
    .fetch_one(&state.db)
    .await?;

    let token = issue_jwt(&user, &state.config.jwt_secret)?;

    Ok(Json(AuthResponse {
        token,
        username: user.username,
        college_tag: college.short_tag,
    }))
}

/// Kept separate from the DB pool param so unit tests can exercise the
/// regex/parsing logic without spinning up MySQL.
#[allow(dead_code)]
fn domain_is_allowed(email: &str) -> bool {
    ALLOWED_EMAIL_RE.is_match(email)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_pilot_domains() {
        assert!(domain_is_allowed("someone@iiitm.ac.in"));
        assert!(domain_is_allowed("Someone.Else@MITSGWALIOR.IN"));
    }

    #[test]
    fn rejects_everything_else() {
        assert!(!domain_is_allowed("someone@gmail.com"));
        assert!(!domain_is_allowed("someone@iiitm.ac.in.evil.com"));
        assert!(!domain_is_allowed("someone@mitsgwalior.in.co"));
    }
}
