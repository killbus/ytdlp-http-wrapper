#![allow(clippy::unwrap_used)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use std::path::PathBuf;
use tower::ServiceExt;
use ytdlp_http_wrapper::routes;

#[tokio::test]
async fn test_get_run_deserialization() {
    let app = routes::app(PathBuf::from("non_existent_ytdlp_binary"));

    // 1. 单参数: GET /run?args=--update
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/run?args=--update")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    // 2. 多参数: GET /run?args=-f&args=best
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/run?args=-f&args=best")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    // 3. 带可选参数: GET /run?args=--update&timeout_seconds=45
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/run?args=--update&timeout_seconds=45")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    // 4. 缺失必填 args 参数: GET /run?timeout_seconds=45
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/run?timeout_seconds=45")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // 5. 完全为空: GET /run
    let response = app
        .clone()
        .oneshot(Request::builder().uri("/run").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
