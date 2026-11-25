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
    pub backend: crate::backend::Backend,
    pub(crate) updater: Option<crate::online_update::OnlineUpdater>,
    pub(crate) shutdown: CancellationToken,
    pub(crate) tasks: TaskTracker,
}

impl AppState {
    pub fn new(binary_path: PathBuf, max_concurrent: usize, temp_dir: Option<PathBuf>) -> Self {
        Self {
            binary_path,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            temp_dir,
            backend: crate::backend::Backend::default_backend(),
            updater: None,
            shutdown: CancellationToken::new(),
            tasks: TaskTracker::new(),
        }
    }

    /// Selects the execution backend. The CLI backend is the default; the
    /// resident backend is only reachable through explicit configuration.
    pub fn with_backend(mut self, backend: crate::backend::Backend) -> Self {
        self.backend = backend;
        self
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

    pub fn with_updates(
        mut self,
        config: crate::online_update::UpdateConfig,
        paths: crate::runtime_installer::RuntimePaths,
        version: String,
    ) -> Self {
        let execution = Arc::new(Self {
            binary_path: self.binary_path.clone(),
            semaphore: self.semaphore.clone(),
            temp_dir: self.temp_dir.clone(),
            backend: self.backend.clone(),
            updater: None,
            shutdown: self.shutdown.clone(),
            tasks: self.tasks.clone(),
        });
        self.updater = Some(crate::online_update::OnlineUpdater::new(
            config, execution, paths, version,
        ));
        self
    }

    pub async fn stop_backend(&self) {
        if let Some(updater) = &self.updater {
            updater.active.read().await.state.backend.shutdown().await;
        } else {
            self.backend.shutdown().await;
        }
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
