#![allow(clippy::unwrap_used)]

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use std::{path::PathBuf, sync::Arc};
use tower::ServiceExt;
use ytdlp_http_wrapper::routes::{self, AppState};

fn app() -> axum::Router {
    routes::app(Arc::new(AppState::new(
        PathBuf::from("non-existent-yt-dlp"),
        1,
        None,
    )))
}

async fn body(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn get_and_post_keep_the_public_request_contract() {
    let get = app()
        .oneshot(
            Request::builder()
                .uri("/run?args=--version&timeout_seconds=45")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body(get).await["code"], "SPAWN_FAILURE");

    let post = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/run")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "args": ["--version"],
                        "timeout_seconds": 45
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(post.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body(post).await["code"], "SPAWN_FAILURE");
}

#[tokio::test]
async fn denied_arguments_are_rejected_before_admission() {
    let state = Arc::new(AppState::new(PathBuf::from("non-existent-yt-dlp"), 1, None));
    let response = routes::app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/run")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"args":["--exec","echo unsafe"]}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let payload = body(response).await;
    assert_eq!(payload["code"], "ARG_REJECTED");
    assert!(payload["error"]
        .as_str()
        .is_some_and(|message| message.contains("--exec")));
    assert_eq!(state.semaphore.available_permits(), 1);
}

#[tokio::test]
async fn malformed_requests_keep_framework_level_bad_request_responses() {
    let missing_args = app()
        .oneshot(
            Request::builder()
                .uri("/run?timeout_seconds=30")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_args.status(), StatusCode::BAD_REQUEST);

    let malformed_json = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/run")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"args":"not-an-array"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(malformed_json.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
