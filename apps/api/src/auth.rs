//! Account registration HTTP handler.

use argon2::{
    Argon2,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use axum::{
    Extension, Json,
    extract::{State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    response::IntoResponse,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool};
use tower_http::request_id::RequestId;
use uuid::Uuid;

use crate::ApiError;

const MIN_PASSWORD_BYTES: usize = 12;
const MAX_PASSWORD_BYTES: usize = 128;
const ACCESS_TOKEN_LIFETIME_SECONDS: i64 = 15 * 60;
const REFRESH_TOKEN_LIFETIME_DAYS: i64 = 30;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RegisterRequest {
    email: String,
    password: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RefreshRequest {
    refresh_token: String,
}

#[derive(Serialize, sqlx::FromRow)]
struct RegisterResponse {
    id: Uuid,
    email: String,
    created_at: DateTime<Utc>,
}

#[derive(Serialize)]
struct TokenResponse {
    access_token: String,
    token_type: &'static str,
    expires_in: i64,
    refresh_token: String,
    user: UserResponse,
}

#[derive(Clone, Serialize, FromRow)]
pub(super) struct UserResponse {
    id: Uuid,
    email: String,
    created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct AccountCredentials {
    id: Uuid,
    email: String,
    password_hash: String,
    disabled_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct RefreshSession {
    id: Uuid,
    user_id: Uuid,
    email: String,
    created_at: DateTime<Utc>,
    disabled_at: Option<DateTime<Utc>>,
    revoked_at: Option<DateTime<Utc>>,
    rotated_to_session_id: Option<Uuid>,
    expires_at: DateTime<Utc>,
}

pub(super) async fn register(
    State(pool): State<PgPool>,
    Extension(request_id): Extension<RequestId>,
    body: Result<Json<RegisterRequest>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Json(payload) = body.map_err(|_error| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_json",
            "Request body must be a valid registration JSON object",
            request_id.header_value().to_str().ok().map(str::to_owned),
        )
    })?;

    let email = normalize_email(&payload.email).ok_or_else(|| {
        ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_email",
            "Email address is invalid",
            request_id.header_value().to_str().ok().map(str::to_owned),
        )
    })?;

    if !(MIN_PASSWORD_BYTES..=MAX_PASSWORD_BYTES).contains(&payload.password.len()) {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_password",
            "Password must be between 12 and 128 bytes",
            request_id.header_value().to_str().ok().map(str::to_owned),
        ));
    }

    let password_hash = tokio::task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(payload.password.as_bytes())
            .map(|hash| hash.to_string())
    })
    .await
    .map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "The request could not be completed",
            request_id.header_value().to_str().ok().map(str::to_owned),
        )
    })?
    .map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "The request could not be completed",
            request_id.header_value().to_str().ok().map(str::to_owned),
        )
    })?;

    let user = sqlx::query_as::<_, RegisterResponse>(
        "INSERT INTO users (email, password_hash) VALUES ($1, $2) \
         RETURNING id, email, created_at",
    )
    .bind(email)
    .bind(password_hash)
    .fetch_one(&pool)
    .await
    .map_err(|error| {
        let duplicate_email = matches!(
            &error,
            sqlx::Error::Database(database_error)
                if database_error.code().as_deref() == Some("23505")
                    && database_error.constraint() == Some("users_email_lower_unique")
        );
        if duplicate_email {
            ApiError::new(
                StatusCode::CONFLICT,
                "email_already_registered",
                "An account with this email address already exists",
                request_id.header_value().to_str().ok().map(str::to_owned),
            )
        } else {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "The request could not be completed",
                request_id.header_value().to_str().ok().map(str::to_owned),
            )
        }
    })?;

    Ok((StatusCode::CREATED, Json(user)))
}

pub(super) async fn login(
    State(pool): State<PgPool>,
    Extension(request_id): Extension<RequestId>,
    body: Result<Json<LoginRequest>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Json(payload) = body.map_err(|_error| {
        api_error(
            StatusCode::BAD_REQUEST,
            "invalid_json",
            "Request body must be a valid login JSON object",
            &request_id,
        )
    })?;
    let Some(email) = normalize_email(&payload.email) else {
        return Err(unauthorized(&request_id));
    };
    if payload.password.len() > MAX_PASSWORD_BYTES {
        return Err(unauthorized(&request_id));
    }

    let account = sqlx::query_as::<_, AccountCredentials>(
        "SELECT id, email, password_hash, disabled_at, created_at \
         FROM users WHERE lower(email) = $1",
    )
    .bind(email)
    .fetch_optional(&pool)
    .await
    .map_err(|_| internal_error(&request_id))?;

    let hash = account.as_ref().map(|user| user.password_hash.clone());
    let password = payload.password;
    let password_matches =
        tokio::task::spawn_blocking(move || verify_password(hash.as_deref(), password.as_bytes()))
            .await
            .map_err(|_| internal_error(&request_id))?;

    let Some(account) = account.filter(|user| user.disabled_at.is_none() && password_matches)
    else {
        return Err(unauthorized(&request_id));
    };

    let user = UserResponse {
        id: account.id,
        email: account.email,
        created_at: account.created_at,
    };
    let tokens = create_session(&pool, user)
        .await
        .map_err(|_| internal_error(&request_id))?;
    Ok((StatusCode::OK, Json(tokens)))
}

pub(super) async fn refresh(
    State(pool): State<PgPool>,
    Extension(request_id): Extension<RequestId>,
    body: Result<Json<RefreshRequest>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Json(payload) = body.map_err(|_error| {
        api_error(
            StatusCode::BAD_REQUEST,
            "invalid_json",
            "Request body must contain a refresh token",
            &request_id,
        )
    })?;
    if !is_token_format(&payload.refresh_token, "gr_") {
        return Err(unauthorized(&request_id));
    }

    let mut transaction = pool
        .begin()
        .await
        .map_err(|_| internal_error(&request_id))?;
    let session = sqlx::query_as::<_, RefreshSession>(
        "SELECT s.id, s.user_id, u.email, u.created_at, u.disabled_at, \
         s.revoked_at, s.rotated_to_session_id, s.expires_at \
         FROM sessions s JOIN users u ON u.id = s.user_id \
         WHERE s.refresh_token_hash = $1 FOR UPDATE OF s",
    )
    .bind(token_hash(&payload.refresh_token).to_vec())
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| internal_error(&request_id))?;

    let Some(session) = session else {
        return Err(unauthorized(&request_id));
    };
    if session.revoked_at.is_some() {
        if session.rotated_to_session_id.is_some() {
            sqlx::query(
                "UPDATE sessions SET revoked_at = COALESCE(revoked_at, now()) \
                 WHERE user_id = $1 AND revoked_at IS NULL",
            )
            .bind(session.user_id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| internal_error(&request_id))?;
            transaction
                .commit()
                .await
                .map_err(|_| internal_error(&request_id))?;
        }
        return Err(unauthorized(&request_id));
    }
    if session.expires_at <= Utc::now() || session.disabled_at.is_some() {
        return Err(unauthorized(&request_id));
    }

    let user = UserResponse {
        id: session.user_id,
        email: session.email,
        created_at: session.created_at,
    };
    let tokens = issue_tokens(user);
    let new_session_id = Uuid::new_v4();
    let now = Utc::now();
    sqlx::query(
        "INSERT INTO sessions \
         (id, user_id, access_token_hash, access_expires_at, refresh_token_hash, expires_at) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(new_session_id)
    .bind(tokens.user.id)
    .bind(token_hash(&tokens.access_token).to_vec())
    .bind(now + Duration::seconds(ACCESS_TOKEN_LIFETIME_SECONDS))
    .bind(token_hash(&tokens.refresh_token).to_vec())
    .bind(now + Duration::days(REFRESH_TOKEN_LIFETIME_DAYS))
    .execute(&mut *transaction)
    .await
    .map_err(|_| internal_error(&request_id))?;

    sqlx::query("UPDATE sessions SET revoked_at = $2, rotated_to_session_id = $3 WHERE id = $1")
        .bind(session.id)
        .bind(now)
        .bind(new_session_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| internal_error(&request_id))?;
    transaction
        .commit()
        .await
        .map_err(|_| internal_error(&request_id))?;

    Ok((StatusCode::OK, Json(tokens)))
}

pub(super) async fn logout(
    State(pool): State<PgPool>,
    Extension(request_id): Extension<RequestId>,
    body: Result<Json<RefreshRequest>, JsonRejection>,
) -> Result<StatusCode, ApiError> {
    let Json(payload) = body.map_err(|_error| {
        api_error(
            StatusCode::BAD_REQUEST,
            "invalid_json",
            "Request body must contain a refresh token",
            &request_id,
        )
    })?;
    if !is_token_format(&payload.refresh_token, "gr_") {
        return Err(unauthorized(&request_id));
    }
    let result = sqlx::query(
        "UPDATE sessions SET revoked_at = now() \
         WHERE refresh_token_hash = $1 AND revoked_at IS NULL",
    )
    .bind(token_hash(&payload.refresh_token).to_vec())
    .execute(&pool)
    .await
    .map_err(|_| internal_error(&request_id))?;
    if result.rows_affected() == 0 {
        return Err(unauthorized(&request_id));
    }
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn current_user(
    State(pool): State<PgPool>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
) -> Result<Json<UserResponse>, ApiError> {
    let access_token = bearer_token(&headers).ok_or_else(|| unauthorized(&request_id))?;
    if !is_token_format(access_token, "ga_") {
        return Err(unauthorized(&request_id));
    }
    let user = sqlx::query_as::<_, UserResponse>(
        "SELECT u.id, u.email, u.created_at FROM users u \
         JOIN sessions s ON s.user_id = u.id \
         WHERE s.access_token_hash = $1 AND s.revoked_at IS NULL \
         AND s.access_expires_at > now() AND u.disabled_at IS NULL",
    )
    .bind(token_hash(access_token).to_vec())
    .fetch_optional(&pool)
    .await
    .map_err(|_| internal_error(&request_id))?;
    user.map(Json).ok_or_else(|| unauthorized(&request_id))
}

fn normalize_email(value: &str) -> Option<String> {
    let email = value.trim();
    if email.len() > 320 || email.chars().any(char::is_whitespace) {
        return None;
    }
    let (local, domain) = email.split_once('@')?;
    if local.is_empty()
        || local.len() > 64
        || domain.is_empty()
        || domain.len() > 255
        || domain.contains('@')
    {
        return None;
    }
    Some(email.to_lowercase())
}

async fn create_session(pool: &PgPool, user: UserResponse) -> Result<TokenResponse, sqlx::Error> {
    let tokens = issue_tokens(user);
    let now = Utc::now();
    sqlx::query(
        "INSERT INTO sessions \
         (user_id, access_token_hash, access_expires_at, refresh_token_hash, expires_at) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(tokens.user.id)
    .bind(token_hash(&tokens.access_token).to_vec())
    .bind(now + Duration::seconds(ACCESS_TOKEN_LIFETIME_SECONDS))
    .bind(token_hash(&tokens.refresh_token).to_vec())
    .bind(now + Duration::days(REFRESH_TOKEN_LIFETIME_DAYS))
    .execute(pool)
    .await?;
    Ok(tokens)
}

fn issue_tokens(user: UserResponse) -> TokenResponse {
    TokenResponse {
        access_token: random_token("ga_"),
        token_type: "Bearer",
        expires_in: ACCESS_TOKEN_LIFETIME_SECONDS,
        refresh_token: random_token("gr_"),
        user,
    }
}

fn random_token(prefix: &str) -> String {
    format!(
        "{prefix}{}{}{}",
        Uuid::new_v4().as_simple(),
        Uuid::new_v4().as_simple(),
        Uuid::new_v4().as_simple()
    )
}

fn token_hash(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

fn is_token_format(token: &str, prefix: &str) -> bool {
    token.starts_with(prefix)
        && token.len() == prefix.len() + 96
        && token[prefix.len()..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

fn verify_password(stored_hash: Option<&str>, password: &[u8]) -> bool {
    if let Some(stored_hash) = stored_hash {
        PasswordHash::new(stored_hash)
            .is_ok_and(|parsed| Argon2::default().verify_password(password, &parsed).is_ok())
    } else {
        let _dummy_work = Argon2::default().hash_password(password).is_ok();
        false
    }
}

fn unauthorized(request_id: &RequestId) -> ApiError {
    api_error(
        StatusCode::UNAUTHORIZED,
        "unauthorized",
        "Invalid credentials or session",
        request_id,
    )
}

fn internal_error(request_id: &RequestId) -> ApiError {
    api_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal_error",
        "The request could not be completed",
        request_id,
    )
}

fn api_error(
    status: StatusCode,
    code: &'static str,
    message: &'static str,
    request_id: &RequestId,
) -> ApiError {
    ApiError::new(
        status,
        code,
        message,
        request_id.header_value().to_str().ok().map(str::to_owned),
    )
}
