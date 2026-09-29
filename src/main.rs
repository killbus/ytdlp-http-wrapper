use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;
use yt_dlp::client::deps::LibraryInstaller;

use ytdlp_http_wrapper::routes;

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
        long = "max-concurrent",
        env = "MAX_CONCURRENT_PROCESSES",
        help = "Max concurrent yt-dlp processes"
    )]
    max_concurrent: Option<usize>,

    #[arg(
        long = "temp-dir",
        env = "YTDLP_TEMP_DIR",
        help = "Existing parent directory for per-request temporary files"
    )]
    temp_dir: Option<PathBuf>,

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
    Err(last_err
        .map(Into::into)
        .unwrap_or_else(|| "yt-dlp installer failed with no captured error".into()))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let result = runtime.block_on(serve());
    // A failed Windows kernel/job wait may leave process-wrap's blocking reaper
    // behind. It must not keep an otherwise drained server alive indefinitely.
    runtime.shutdown_timeout(Duration::from_secs(2));
    result
}

async fn serve() -> Result<(), Box<dyn std::error::Error>> {
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

    let max_concurrent = cli.max_concurrent.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|n| n.get() * 2)
            .unwrap_or(8)
    });
    info!(max_concurrent, "Semaphore limit set");

    if max_concurrent == 0 {
        return Err("max-concurrent must be greater than zero".into());
    }
    let temp_dir = cli.temp_dir.map(std::fs::canonicalize).transpose()?;
    if temp_dir.as_ref().is_some_and(|path| !path.is_dir()) {
        return Err("temp-dir must be an existing directory".into());
    }

    info!("Bootstrapping yt-dlp dependency");
    let installer = LibraryInstaller::new(cli.libs_dir);
    let ytdlp_binary_path = install_with_retry(&installer, 3).await?;
    info!(path = %ytdlp_binary_path.display(), "Dependency ready");

    let app_state = Arc::new(routes::AppState::new(
        ytdlp_binary_path,
        max_concurrent,
        temp_dir,
    ));

    let app = routes::app(app_state.clone());

    let addr: SocketAddr = format!("{}:{}", cli.host, cli.port).parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "HTTP server started");
    #[cfg(unix)]
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let shutdown_state = app_state.clone();
    let server = axum::serve(listener, app).with_graceful_shutdown(async move {
        #[cfg(unix)]
        tokio::select! {
            result = tokio::signal::ctrl_c() => { if let Err(e) = result { warn!(error = %e, "Ctrl-C listener failed"); } },
            _ = terminate.recv() => {},
            _ = shutdown_state.shutdown_requested() => {},
        }
        #[cfg(not(unix))]
        tokio::select! {
            result = tokio::signal::ctrl_c() => { if let Err(e) = result { warn!(error = %e, "Ctrl-C listener failed"); } },
            _ = shutdown_state.shutdown_requested() => {},
        }
        info!("Shutdown requested; cancelling active processes");
        shutdown_state.begin_shutdown();
    }).await;
    app_state.begin_shutdown();
    app_state.wait_for_cleanup().await;
    server?;

    Ok(())
}
