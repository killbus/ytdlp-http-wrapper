use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;
use yt_dlp::client::deps::LibraryInstaller;

mod executor;
mod models;
mod routes;

async fn install_with_retry(
    installer: &LibraryInstaller,
    max_retries: u32,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut last_err = None;
    for attempt in 1..=max_retries {
        match installer.install_youtube(None).await {
            Ok(path) => return Ok(path),
            Err(e) => {
                warn!(
                    attempt,
                    error = %e,
                    "yt-dlp install attempt failed"
                );
                last_err = Some(e);
                if attempt < max_retries {
                    tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
                }
            }
        }
    }
    Err(last_err.unwrap().into())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    if std::env::args().any(|a| a == "--help" || a == "-h") {
        eprintln!("ytdlp-http-wrapper — HTTP wrapper for yt-dlp");
        eprintln!();
        eprintln!("Environment variables:");
        eprintln!("  HOST        Server bind address (default: 127.0.0.1)");
        eprintln!("  PORT        Server port (default: 8080)");
        eprintln!("  LIBS_DIR    yt-dlp download directory (default: libs)");
        eprintln!("  DENIED_ARGS Blocked argument patterns (default: built-in list)");
        eprintln!("  RUST_LOG    Logging filter (default: info)");
        return Ok(());
    }

    let destination = env::var("LIBS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("libs"));

    info!("Bootstrapping yt-dlp dependency");
    let installer = LibraryInstaller::new(destination.clone());
    let ytdlp_binary_path = install_with_retry(&installer, 3).await?;

    info!(path = %ytdlp_binary_path.display(), "Dependency ready");

    let app = routes::app(ytdlp_binary_path);

    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;

    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "HTTP server started");
    axum::serve(listener, app).await?;

    Ok(())
}
