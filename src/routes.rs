use axum::extract::State;
use axum::{http::Request, response::IntoResponse, routing::get, Json, Router};
use axum_extra::extract::Query;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tower_http::trace::TraceLayer;
use tracing::Span;

use crate::executor;
use crate::models::RunRequest;

pub struct AppState {
    pub binary_path: PathBuf,
    pub semaphore: Arc<Semaphore>,
    pub temp_dir: Option<PathBuf>,
    pub(crate) shutdown: CancellationToken,
    pub(crate) tasks: TaskTracker,
}

impl AppState {
    pub fn new(binary_path: PathBuf, max_concurrent: usize, temp_dir: Option<PathBuf>) -> Self {
        Self {
            binary_path,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            temp_dir,
            shutdown: CancellationToken::new(),
            tasks: TaskTracker::new(),
        }
    }

    pub fn begin_shutdown(&self) {
        self.semaphore.close();
        self.shutdown.cancel();
    }

    pub async fn shutdown_requested(&self) {
        self.shutdown.cancelled().await;
    }

    /// Call after the HTTP server has stopped admitting handlers.
    pub async fn wait_for_cleanup(&self) {
        self.tasks.close();
        self.tasks.wait().await;
    }
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
