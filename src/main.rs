use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use ytdlp_http_wrapper::routes;
use ytdlp_http_wrapper::runtime_installer::{ensure_runtime, RuntimeConfig, RuntimePaths};

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
        help = "Private installation root containing runtime/ and staging/"
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

    #[arg(
        long = "backend",
        env = "BACKEND",
        default_value = "cli",
        help = "Execution backend: cli (default) or resident"
    )]
    backend: String,

    #[arg(
        long = "worker-python",
        env = "WORKER_PYTHON",
        help = "Override the installed runtime's Python interpreter (resident only)"
    )]
    worker_python: Option<PathBuf>,

    #[arg(
        long = "worker-path",
        env = "WORKER_PATH",
        help = "Override the installed worker script (resident only)"
    )]
    worker_path: Option<PathBuf>,

    #[arg(
        long = "worker-python-path",
        env = "WORKER_PYTHONPATH",
        help = "PATH-SEPARATOR-separated PYTHONPATH entries for the resident worker"
    )]
    worker_python_path: Option<String>,

    #[arg(
        long = "worker-yt-dlp-version",
        env = "WORKER_YTDLP_VERSION",
        help = "Worker handshake version; defaults to and must equal the runtime yt-dlp pin"
    )]
    worker_yt_dlp_version: Option<String>,

    #[arg(
        long = "worker-pool-size",
        env = "WORKER_POOL_SIZE",
        default_value = "1",
        help = "Resident worker pool capacity (number of workers); must be at least 1"
    )]
    worker_pool_size: usize,

    #[arg(
        long = "runtime-yt-dlp-version",
        env = "RUNTIME_YTDLP_VERSION",
        default_value = "2026.09.23",
        help = "Pinned yt-dlp version installed into the immutable runtime"
    )]
    runtime_yt_dlp_version: String,

    #[arg(
        long = "runtime-python-release",
        env = "RUNTIME_PYTHON_RELEASE",
        default_value = "20261026",
        help = "python-build-standalone release tag for the runtime interpreter"
    )]
    runtime_python_release: String,

    #[arg(
        long = "runtime-plan",
        env = "RUNTIME_PLAN",
        help = "Optional runtime source plan JSON (file:// URLs are supported for offline tests)"
    )]
    runtime_plan: Option<PathBuf>,
}

fn build_backend(
    cli: &Cli,
    runtime_paths: &RuntimePaths,
    temp_dir: Option<PathBuf>,
) -> Result<ytdlp_http_wrapper::backend::Backend, Box<dyn std::error::Error>> {
    match cli.backend.as_str() {
        // The CLI backend stays the default.
        "cli" => Ok(ytdlp_http_wrapper::backend::Backend::default_backend()),
        "resident" => {
            let python = cli
                .worker_python
                .clone()
                .unwrap_or_else(|| runtime_paths.python_path.clone());
            let worker_path = cli
                .worker_path
                .clone()
                .unwrap_or_else(|| runtime_paths.worker_path.clone());
            let mut python_path_entries = runtime_paths.python_path_entries.clone();
            python_path_entries.extend(
                cli.worker_python_path
                    .as_deref()
                    .filter(|value| !value.is_empty())
                    .map(|value| std::env::split_paths(value).collect::<Vec<_>>())
                    .unwrap_or_default(),
            );
            let expected_version = cli
                .worker_yt_dlp_version
                .clone()
                .unwrap_or_else(|| cli.runtime_yt_dlp_version.clone());
            ytdlp_http_wrapper::backend::Backend::from_runtime_config(
                "resident",
                python,
                worker_path,
                python_path_entries,
                &expected_version,
                temp_dir,
                cli.worker_pool_size,
                &runtime_paths.runtime_id,
            )
            .map_err(Into::into)
        }
        other => Err(format!("unknown backend '{other}' (expected cli or resident)").into()),
    }
}

async fn install_runtime(cli: &Cli) -> Result<RuntimePaths, Box<dyn std::error::Error>> {
    ensure_runtime(RuntimeConfig {
        libs_dir: cli.libs_dir.clone(),
        plan_path: cli.runtime_plan.clone(),
        yt_dlp_version: cli.runtime_yt_dlp_version.clone(),
        python_release_tag: cli.runtime_python_release.clone(),
        worker_yt_dlp_version: cli.worker_yt_dlp_version.clone(),
    })
    .await
    .map_err(Into::into)
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
    // Boot-time pool validation: 0 fails here (a non-numeric value is
    // rejected by clap at parse), before any request is admitted, in both
    // backends (design.md section 8).
    if cli.worker_pool_size < 1 {
        return Err(
            "worker-pool-size must be at least 1 (got 0); set --worker-pool-size or WORKER_POOL_SIZE"
                .into(),
        );
    }
    let temp_dir = cli
        .temp_dir
        .clone()
        .map(std::fs::canonicalize)
        .transpose()?;
    if temp_dir.as_ref().is_some_and(|path| !path.is_dir()) {
        return Err("temp-dir must be an existing directory".into());
    }

    let runtime_paths = install_runtime(&cli).await?;
    info!(
        runtime_id = %runtime_paths.runtime_id,
        binary = %runtime_paths.binary_path.display(),
        "immutable runtime ready"
    );

    let backend = build_backend(&cli, &runtime_paths, temp_dir.clone())?;
    // A bad resident configuration fails at boot, before any request is admitted.
    backend.start().await?;
    info!(backend = %backend.name(), "execution backend ready");

    let app_state = Arc::new(
        routes::AppState::new(runtime_paths.binary_path.clone(), max_concurrent, temp_dir)
            .with_backend(backend.clone()),
    );

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
    app_state.backend.shutdown().await;
    server?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::*;
    use clap::{CommandFactory, FromArgMatches};
    use std::fs;
    use std::process::Command;
    use ytdlp_http_wrapper::backend::Backend;
    use ytdlp_http_wrapper::runtime_installer::RuntimeInstallError;

    fn cli(args: &[&str]) -> Cli {
        // Keep in-process tests independent of the caller's service environment.
        let matches = Cli::command()
            .mut_args(|arg| arg.env(None::<&str>))
            .try_get_matches_from(std::iter::once("wrapper").chain(args.iter().copied()))
            .unwrap();
        Cli::from_arg_matches(&matches).unwrap()
    }

    fn paths(root: &std::path::Path) -> RuntimePaths {
        RuntimePaths {
            binary_path: root.join("runtime/bin/unused-yt-dlp"),
            python_path: root.join("runtime/python/missing-python"),
            worker_path: root.join("runtime/worker/ytdlp_worker.py"),
            python_path_entries: vec![root.join("runtime/python/site-packages")],
            runtime_id: "runtime-fixture-identity".into(),
        }
    }

    #[test]
    fn defaults_keep_cli_and_pinned_runtime() {
        let cli = cli(&[]);
        assert_eq!(cli.libs_dir, PathBuf::from("libs"));
        assert_eq!(cli.runtime_yt_dlp_version, "2026.09.23");
        assert_eq!(cli.runtime_python_release, "20261026");
        assert!(cli.runtime_plan.is_none());
        assert!(cli.worker_python.is_none());
        assert!(cli.worker_path.is_none());
        assert!(cli.worker_python_path.is_none());
        assert!(cli.worker_yt_dlp_version.is_none());
        assert_eq!(cli.worker_pool_size, 1);
        assert_eq!(cli.backend, "cli");
        let root = tempfile::tempdir().unwrap();
        assert!(matches!(
            build_backend(&cli, &paths(root.path()), None).unwrap(),
            Backend::Cli
        ));
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }

    #[test]
    fn flags_override_environment() {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args([
            "--exact",
            "tests::configuration_environment_fixture",
            "--nocapture",
        ]);
        for arg in Cli::command().get_arguments() {
            if let Some(name) = arg.get_env() {
                command.env_remove(name);
            }
        }
        command
            .env("YTDLP_BOOT_CONFIG_FIXTURE", "1")
            .env("BACKEND", "resident")
            .env("LIBS_DIR", "env-libs")
            .env("RUNTIME_YTDLP_VERSION", "env-version")
            .env("RUNTIME_PYTHON_RELEASE", "20261026")
            .env("RUNTIME_PLAN", "env-plan.json")
            .env("WORKER_PYTHON", "env-python")
            .env("WORKER_PATH", "env-worker.py")
            .env("WORKER_PYTHONPATH", "env-packages")
            .env("WORKER_YTDLP_VERSION", "env-version")
            .env("WORKER_POOL_SIZE", "2");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let output = command.output().unwrap();
        assert!(output.status.success(), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("configuration environment checked")
        );
    }

    #[test]
    fn configuration_environment_fixture() {
        if std::env::var("YTDLP_BOOT_CONFIG_FIXTURE").as_deref() != Ok("1") {
            return;
        }
        let cli = Cli::try_parse_from(["wrapper"]).unwrap();
        assert_eq!(cli.backend, "resident");
        assert_eq!(cli.libs_dir, PathBuf::from("env-libs"));
        assert_eq!(cli.runtime_yt_dlp_version, "env-version");
        assert_eq!(cli.runtime_python_release, "20261026");
        assert_eq!(cli.runtime_plan, Some("env-plan.json".into()));
        assert_eq!(cli.worker_python, Some("env-python".into()));
        assert_eq!(cli.worker_path, Some("env-worker.py".into()));
        assert_eq!(cli.worker_python_path.as_deref(), Some("env-packages"));
        assert_eq!(cli.worker_yt_dlp_version.as_deref(), Some("env-version"));
        assert_eq!(cli.worker_pool_size, 2);

        let cli = Cli::try_parse_from([
            "wrapper",
            "--backend",
            "cli",
            "--libs-dir",
            "flag-libs",
            "--runtime-yt-dlp-version",
            "flag-version",
            "--runtime-python-release",
            "20260101",
            "--runtime-plan",
            "flag-plan.json",
            "--worker-python",
            "flag-python",
            "--worker-path",
            "flag-worker.py",
            "--worker-python-path",
            "flag-packages",
            "--worker-yt-dlp-version",
            "flag-version",
            "--worker-pool-size",
            "3",
        ])
        .unwrap();
        assert_eq!(cli.backend, "cli");
        assert_eq!(cli.libs_dir, PathBuf::from("flag-libs"));
        assert_eq!(cli.runtime_yt_dlp_version, "flag-version");
        assert_eq!(cli.runtime_python_release, "20260101");
        assert_eq!(cli.runtime_plan, Some("flag-plan.json".into()));
        assert_eq!(cli.worker_python, Some("flag-python".into()));
        assert_eq!(cli.worker_path, Some("flag-worker.py".into()));
        assert_eq!(cli.worker_python_path.as_deref(), Some("flag-packages"));
        assert_eq!(cli.worker_yt_dlp_version.as_deref(), Some("flag-version"));
        assert_eq!(cli.worker_pool_size, 3);
        println!("configuration environment checked");
    }

    #[tokio::test]
    async fn worker_version_mismatch_stops_both_backends_before_resolution() {
        for backend in ["cli", "resident"] {
            let root = tempfile::tempdir().unwrap();
            let mut cli = cli(&["--backend", backend, "--worker-yt-dlp-version", "different"]);
            cli.libs_dir = root.path().join("not-created");
            cli.runtime_plan = Some(root.path().join("missing-plan.json"));
            let error = install_runtime(&cli).await.unwrap_err();
            assert!(format!("{error:?}").contains("RUNTIME_VERSION_MISMATCH"));
            let error = error.downcast_ref::<RuntimeInstallError>().unwrap();
            assert_eq!(error.code(), "RUNTIME_VERSION_MISMATCH");
            assert!(error.to_string().contains("attempt=1"));
            assert!(!cli.libs_dir.exists());
        }
    }

    #[tokio::test]
    async fn resident_paths_overrides_and_package_order_reach_handshake() {
        let python = ["python", "python3"]
            .into_iter()
            .find_map(|name| {
                Command::new(name)
                    .args(["-c", "import sys; print(sys.executable)"])
                    .output()
                    .ok()
                    .filter(|output| output.status.success())
                    .map(|output| PathBuf::from(String::from_utf8(output.stdout).unwrap().trim()))
            })
            .expect("Python is required for the resident boot wiring test");

        for mode in [
            "defaults",
            "empty-python-path",
            "overrides",
            "wrong-handshake",
        ] {
            let root = tempfile::tempdir().unwrap();
            let mut runtime = paths(root.path());
            let mut cli = cli(&[
                "--backend",
                "resident",
                "--runtime-yt-dlp-version",
                "fixture-version",
            ]);
            let package = runtime.python_path_entries[0].join("yt_dlp");
            fs::create_dir_all(&package).unwrap();
            fs::write(
                package.join("__init__.py"),
                include_bytes!("../tests/fixtures/yt_dlp/__init__.py"),
            )
            .unwrap();

            let worker = if mode == "overrides" {
                let worker = root.path().join("override-worker.py");
                cli.worker_python = Some(python.clone());
                cli.worker_path = Some(worker.clone());
                cli.worker_yt_dlp_version = Some(cli.runtime_yt_dlp_version.clone());
                cli.worker_pool_size = 2;
                let extra = root.path().join("extra packages");
                fs::create_dir_all(extra.join("yt_dlp")).unwrap();
                fs::write(
                    extra.join("yt_dlp/__init__.py"),
                    "raise RuntimeError('runtime must come first')\n",
                )
                .unwrap();
                fs::write(extra.join("boot_path_probe.py"), "").unwrap();
                let second_extra = root.path().join("second packages");
                fs::create_dir(&second_extra).unwrap();
                fs::write(second_extra.join("boot_second_probe.py"), "").unwrap();
                let mut package_source =
                    include_str!("../tests/fixtures/yt_dlp/__init__.py").to_owned();
                package_source.push_str("\nimport boot_path_probe, boot_second_probe\n");
                fs::write(package.join("__init__.py"), package_source).unwrap();
                cli.worker_python_path = Some(
                    std::env::join_paths([extra, second_extra])
                        .unwrap()
                        .into_string()
                        .unwrap(),
                );
                // Runtime interpreter/script stay nonexistent: explicit overrides must win.
                worker
            } else {
                runtime.python_path = python.clone();
                fs::create_dir_all(runtime.worker_path.parent().unwrap()).unwrap();
                if mode == "empty-python-path" {
                    cli.worker_python_path = Some(String::new());
                } else if mode == "wrong-handshake" {
                    cli.runtime_yt_dlp_version = "not-the-imported-version".into();
                }
                runtime.worker_path.clone()
            };
            fs::write(worker, include_bytes!("../worker/ytdlp_worker.py")).unwrap();
            let backend = build_backend(&cli, &runtime, None).unwrap();
            let Backend::Resident(Some(manager)) = &backend else {
                panic!("resident requested")
            };
            assert_eq!(manager.generation_id(), runtime.runtime_id);
            let result = backend.start().await;
            backend.shutdown().await;
            if mode == "wrong-handshake" {
                assert!(result.unwrap_err().contains("WORKER_HANDSHAKE_FAILURE"));
            } else {
                result.unwrap();
            }
            assert_eq!(backend.name(), "resident");
        }
    }
}
