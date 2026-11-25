#![allow(clippy::unwrap_used)]

use axum::{
    extract::FromRequestParts,
    http::{Request, StatusCode},
    response::IntoResponse,
};
use axum_extra::extract::Query;
use ytdlp_http_wrapper::models::RunRequest;

#[tokio::test]
async fn test_get_run_deserialization() {
    for (query, args, timeout_seconds) in [
        ("/run?args=--update", vec!["--update"], None),
        ("/run?args=-f&args=best", vec!["-f", "best"], None),
        (
            "/run?args=--update&timeout_seconds=45",
            vec!["--update"],
            Some(45),
        ),
    ] {
        let (mut parts, ()) = Request::builder().uri(query).body(()).unwrap().into_parts();
        let Query(request) = Query::<RunRequest>::from_request_parts(&mut parts, &())
            .await
            .unwrap();
        assert_eq!(request.args, args, "{query}");
        assert_eq!(request.timeout_seconds, timeout_seconds, "{query}");
    }
}

#[tokio::test]
async fn missing_args_and_invalid_timeout_are_bad_requests() {
    for query in [
        "/run?timeout_seconds=45",
        "/run",
        "/run?args=--update&timeout_seconds=invalid",
    ] {
        let (mut parts, ()) = Request::builder().uri(query).body(()).unwrap().into_parts();
        match Query::<RunRequest>::from_request_parts(&mut parts, &()).await {
            Err(error) => assert_eq!(
                error.into_response().status(),
                StatusCode::BAD_REQUEST,
                "{query}"
            ),
            Ok(_) => panic!("invalid query was accepted: {query}"),
        }
    }
}
