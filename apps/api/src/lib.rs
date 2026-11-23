use std::{net::SocketAddr, num::NonZeroU32, time::Duration};

use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Extension, Request, State},
    http::{HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Serialize;
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower::ServiceBuilder;
use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder};
use tower_http::{
    cors::CorsLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, RequestId, SetRequestIdLayer},
    trace::TraceLayer,
};
use tracing::info_span;

mod auth;
pub mod contracts;

#[derive(Clone)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub allowed_origin: HeaderValue,
    database_url: String,
    requests_per_second: NonZeroU32,
    burst_size: NonZeroU32,
}

impl Config {
    /// Read API settings from the process environment.
    ///
    /// # Errors
    /// Returns an error if `DATABASE_URL` is missing, or if an address, origin, or rate limit is invalid.
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let bind_addr = env_or("API_BIND_ADDR", "127.0.0.1:8080").parse()?;
        let allowed_origin =
            HeaderValue::from_str(&env_or("API_ALLOWED_ORIGIN", "http://localhost:3000"))?;
        let database_url = std::env::var("DATABASE_URL")
            .map_err(|_| "DATABASE_URL must be set to connect to PostgreSQL")?;
        let requests_per_second = parse_nonzero("API_RATE_LIMIT_PER_SECOND", "5")?;
        if u32::from(requests_per_second) > 1_000 {
            return Err("API_RATE_LIMIT_PER_SECOND must not exceed 1000".into());
        }
        let burst_size = parse_nonzero("API_RATE_LIMIT_BURST", "20")?;

        Ok(Self {
            bind_addr,
            allowed_origin,
            database_url,
            requests_per_second,
            burst_size,
        })
    }
}

/// Construct the API router and its request middleware.
///
/// # Errors
/// Returns an error if the rate-limit settings cannot form a governor quota.
pub fn app(config: &Config, pool: PgPool) -> Result<Router, std::io::Error> {
    let rate_limit = GovernorConfigBuilder::default()
        .per_millisecond(u64::from((1_000 / config.requests_per_second).max(1)))
        .burst_size(config.burst_size.get())
        .finish()
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid rate limit configuration",
            )
        })?;

    let limiter = rate_limit.limiter().clone();
    let login_rate_limit = GovernorConfigBuilder::default()
        .per_second(12)
        .burst_size(5)
        .finish()
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid login rate limit configuration",
            )
        })?;
    let login_limiter = login_rate_limit.limiter().clone();
    tokio::spawn(async move {
        let mut cleanup = tokio::time::interval(Duration::from_secs(60));
        loop {
            cleanup.tick().await;
            limiter.retain_recent();
            login_limiter.retain_recent();
        }
    });

    let cors = CorsLayer::new()
        .allow_origin(config.allowed_origin.clone())
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::HeaderName::from_static("x-request-id"),
        ])
        .max_age(Duration::from_secs(600));

    let login_route = Router::new()
        .route("/api/v1/auth/login", axum::routing::post(auth::login))
        .route_layer(
            GovernorLayer::new(login_rate_limit).error_handler(|_error| {
                ApiError::new(
                    StatusCode::TOO_MANY_REQUESTS,
                    "rate_limited",
                    "Login rate limit exceeded",
                    None,
                )
                .into_response()
            }),
        );

    Ok(Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/openapi.yaml", get(openapi))
        .route("/api/v1/auth/register", axum::routing::post(auth::register))
        .route("/api/v1/auth/refresh", axum::routing::post(auth::refresh))
        .route("/api/v1/auth/logout", axum::routing::post(auth::logout))
        .route("/api/v1/users/me", get(auth::current_user))
        .merge(login_route)
        .fallback(not_found)
        .method_not_allowed_fallback(method_not_allowed)
        .layer(
            ServiceBuilder::new()
                .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
                .layer(PropagateRequestIdLayer::x_request_id())
                .layer(
                    TraceLayer::new_for_http().make_span_with(|request: &Request<Body>| {
                        info_span!(
                            "http_request",
                            method = %request.method(),
                            path = %request.uri().path(),
                        )
                    }),
                )
                .layer(cors)
                .layer(DefaultBodyLimit::max(16 * 1024))
                .layer(GovernorLayer::new(rate_limit).error_handler(|_error| {
                    ApiError::new(
                        StatusCode::TOO_MANY_REQUESTS,
                        "rate_limited",
                        "Request rate limit exceeded",
                        None,
                    )
                    .into_response()
                })),
        )
        .with_state(pool))
}

async fn openapi() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/yaml; charset=utf-8")],
        include_str!("../../../docs/api/openapi.yaml"),
    )
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    version: &'static str,
}

async fn health(
    State(pool): State<PgPool>,
    Extension(request_id): Extension<RequestId>,
) -> Result<Json<HealthResponse>, ApiError> {
    sqlx::query("SELECT 1").execute(&pool).await.map_err(|_| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "database_unavailable",
            "The database is unavailable",
            request_id.header_value().to_str().ok().map(str::to_owned),
        )
    })?;

    Ok(Json(HealthResponse {
        status: "ok",
        service: "gta6rp-api",
        version: env!("CARGO_PKG_VERSION"),
    }))
}

async fn not_found(request: Request) -> Response {
    error_for_request(
        &request,
        StatusCode::NOT_FOUND,
        "not_found",
        "The requested resource was not found",
    )
}

async fn method_not_allowed(request: Request) -> Response {
    error_for_request(
        &request,
        StatusCode::METHOD_NOT_ALLOWED,
        "method_not_allowed",
        "The HTTP method is not allowed for this resource",
    )
}

fn error_for_request(
    request: &Request,
    status: StatusCode,
    code: &'static str,
    message: &'static str,
) -> Response {
    let request_id = request_id(request);

    ApiError::new(status, code, message, request_id).into_response()
}

fn request_id(request: &Request) -> Option<String> {
    request
        .extensions()
        .get::<RequestId>()
        .and_then(|id| id.header_value().to_str().ok())
        .map(str::to_owned)
}

pub(crate) struct ApiError {
    status: StatusCode,
    body: contracts::ErrorEnvelope,
}

impl ApiError {
    pub(crate) fn new(
        status: StatusCode,
        code: &'static str,
        message: &'static str,
        request_id: Option<String>,
    ) -> Self {
        Self {
            status,
            body: contracts::ErrorEnvelope {
                error: contracts::ErrorDetails {
                    code,
                    message,
                    request_id,
                },
            },
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_owned())
}

fn parse_nonzero(
    name: &str,
    default: &str,
) -> Result<NonZeroU32, Box<dyn std::error::Error + Send + Sync>> {
    let value = env_or(name, default).parse::<u32>()?;
    NonZeroU32::new(value).ok_or_else(|| format!("{name} must be greater than zero").into())
}

/// Bind the configured listener and serve requests until Ctrl+C is received.
///
/// # Errors
/// Returns an error if the listener cannot bind or serving the API fails.
pub async fn serve(config: Config) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&config.database_url)
        .await?;
    sqlx::migrate!().run(&pool).await?;

    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    tracing::info!(address = %config.bind_addr, "API listener started");

    axum::serve(
        listener,
        app(&config, pool)?.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    Ok(())
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "failed to listen for shutdown signal");
    }
    tracing::info!("shutdown signal received");
}
