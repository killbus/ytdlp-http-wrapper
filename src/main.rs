use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;
use yt_dlp::client::deps::LibraryInstaller;

mod executor;
mod models;
mod routes;

#[derive(Parser)]
#[command(name = "ytdlp-http-wrapper", about = "HTTP wrapper for yt-dlp")]
struct Cli {
    #[arg(
        long = "host",
        env = "HOST",
        default_value = "127.0.0.1",
        help = "Server bind address"
    )]
    host: String,

    #[arg(
        short = 'p',
        long = "port",
        env = "PORT",
        default_value = "8080",
        help = "Server port"
    )]
    port: u16,

    #[arg(
        short = 'l',
        long = "libs-dir",
        env = "LIBS_DIR",
        default_value = "libs",
        help = "yt-dlp download directory"
    )]
    libs_dir: PathBuf,

    #[arg(
        long = "denied-args",
        env = "DENIED_ARGS",
        help = "JSON array of blocked arguments; empty array allows all"
    )]
    denied_args: Option<String>,
}

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

    let cli = Cli::parse();
    info!(
        host = %cli.host,
        port = cli.port,
        libs_dir = %cli.libs_dir.display(),
        "starting ytdlp-http-wrapper"
    );

    if let Some(ref denied) = cli.denied_args {
        info!(denied_args = %denied, "DENIED_ARGS configured");
    }

    info!("Bootstrapping yt-dlp dependency");
    let installer = LibraryInstaller::new(cli.libs_dir);
    let ytdlp_binary_path = install_with_retry(&installer, 3).await?;

    info!(path = %ytdlp_binary_path.display(), "Dependency ready");

    let app = routes::app(ytdlp_binary_path);

    let addr: SocketAddr = format!("{}:{}", cli.host, cli.port).parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "HTTP server started");
    axum::serve(listener, app).await?;

    Ok(())
}
