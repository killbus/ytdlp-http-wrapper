use axum::{extract::Query, http::Request, routing::get, Json, Router};
use std::path::PathBuf;
use std::time::Duration;
use tower_http::trace::TraceLayer;
use tracing::Span;

use crate::executor;
use crate::models::RunRequest;

pub fn app(binary_path: PathBuf) -> Router {
    let path_for_get = binary_path.clone();
    Router::new()
        .route(
            "/run",
            get(move |query: Query<RunRequest>| async move {
                executor::execute(query.0, &path_for_get).await
            })
            .post(move |Json(payload): Json<RunRequest>| async move {
                executor::execute(payload, &binary_path).await
            }),
        )
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
}
