#![cfg(target_os = "linux")]
//! Run explicitly with YTDLP_SMOKE_BINARY pointing to the official Linux onefile.
use axum::{body::to_bytes, http::StatusCode, response::IntoResponse};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::{
    net::TcpListener,
    time::{sleep, timeout},
};
use ytdlp_http_wrapper::{executor, models::RunRequest, routes::AppState};

#[tokio::test]
#[ignore = "requires the official yt-dlp_linux artifact; see scripts/linux-smoke.sh"]
async fn real_onefile_timeout_reclaims_extraction(
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let binary =
        PathBuf::from(std::env::var_os("YTDLP_SMOKE_BINARY").ok_or("YTDLP_SMOKE_BINARY missing")?)
            .canonicalize()?;
    let parent = tempfile::tempdir()?;
    let state = Arc::new(AppState::new(binary, 1, Some(parent.path().into())));
    for _ in 0..3 {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let url = format!("http://{}/video", listener.local_addr()?);
        // Accept HTTP but never answer, so no public website or download is needed.
        let upstream = tokio::spawn(async move {
            let (socket, _) = listener.accept().await?;
            sleep(Duration::from_secs(20)).await;
            drop(socket);
            Ok::<(), std::io::Error>(())
        });
        let process = tokio::spawn(executor::execute(
            RunRequest {
                args: vec![
                    "--ignore-config".into(),
                    "--no-playlist".into(),
                    "--socket-timeout".into(),
                    "60".into(),
                    url,
                ],
                timeout_seconds: Some(3),
            },
            state.clone(),
        ));
        // Prove this run actually used PyInstaller extraction before asserting cleanup.
        let extracted = timeout(Duration::from_secs(3), async {
            loop {
                for request in std::fs::read_dir(parent.path())? {
                    for entry in std::fs::read_dir(request?.path())? {
                        if entry?.file_name().to_string_lossy().starts_with("_MEI") {
                            return Ok::<(), std::io::Error>(());
                        }
                    }
                }
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await;
        let response = timeout(Duration::from_secs(10), process)
            .await??
            .into_response();
        upstream.abort();
        let _ = upstream.await;
        extracted??;
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 25 * 1024 * 1024).await?)?;
        assert_eq!(body["exit_code"], -1, "{body}");
        assert_eq!(
            std::fs::read_dir(parent.path())?.count(),
            0,
            "Extraction residue survived"
        );
        assert_eq!(state.semaphore.available_permits(), 1);
    }
    Ok(())
}
