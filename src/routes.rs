use axum::extract::State;
use axum::{http::Request, response::IntoResponse, routing::get, Json, Router};
use axum_extra::extract::Query;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tower_http::trace::TraceLayer;
use tracing::Span;

use crate::executor;
use crate::models::RunRequest;

pub struct AppState {
    pub binary_path: PathBuf,
    pub semaphore: Arc<Semaphore>,
}

async fn run_get(
    State(state): State<Arc<AppState>>,
    Query(payload): Query<RunRequest>,
) -> impl IntoResponse {
    executor::execute(payload, state).await
}

async fn run_post(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RunRequest>,
) -> impl IntoResponse {
    executor::execute(payload, state).await
}

pub fn app(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/run", get(run_get).post(run_post))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<_>| {
                    tracing::debug_span!(
                        "request",
                        method = %request.method(),
                        uri = %request.uri(),
                        status = tracing::field::Empty,
                    )
                })
                .on_response(
                    |response: &axum::http::Response<_>, _latency: Duration, span: &Span| {
                        span.record("status", response.status().as_u16());
                    },
                ),
        )
        .with_state(state)
}
