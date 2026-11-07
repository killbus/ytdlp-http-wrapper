//! Immutable yt-dlp runtime installation.
//!
//! This module is a leaf owned by boot. It resolves and pins a yt-dlp binary,
//! a python-build-standalone CPython runtime, and the yt_dlp wheel; downloads
//! them into owned staging; and replaces the single `runtime/` before serving.
//! The private root requires stop/await/start deployment. Retirement followed
//! by rename is not a crash-atomic transaction and retains no old runtime.
//!
//! General-purpose crates (`sha2`, `zip`) provide hashing and ZIP extraction.
//! The installer owns orchestration and integrity policy; it does not
//! reimplement SHA-256, DEFLATE, or ZIP parsing.

use std::fmt;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

use serde_json::{json, Value as JsonValue};
use sha2::{Digest, Sha256};

const WORKER_SCRIPT: &[u8] = include_bytes!("../worker/ytdlp_worker.py");
const WORKER_SCRIPT_NAME: &str = "ytdlp_worker.py";
const MANIFEST_NAME: &str = "manifest.json";
const ARTIFACTS_DIR: &str = "artifacts";
const DEFAULT_MAX_ARTIFACT_BYTES: u64 = 512 * 1024 * 1024;
const DEFAULT_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(300);
const DOWNLOAD_ATTEMPTS: u32 = 3;
const MANIFEST_SCHEMA_VERSION: i64 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeErrorCode {
    ConfigInvalid,
    VersionMismatch,
    CurlMissing,
    DownloadFailure,
    ResolveFailure,
    IntegrityFailure,
    ExtractFailure,
    PermissionFailure,
    PublishFailure,
    StagingFailure,
}

impl RuntimeErrorCode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ConfigInvalid => "RUNTIME_CONFIG_INVALID",
            Self::VersionMismatch => "RUNTIME_VERSION_MISMATCH",
            Self::CurlMissing => "RUNTIME_CURL_MISSING",
            Self::DownloadFailure => "RUNTIME_DOWNLOAD_FAILURE",
            Self::ResolveFailure => "RUNTIME_RESOLVE_FAILURE",
            Self::IntegrityFailure => "RUNTIME_INTEGRITY_FAILURE",
            Self::ExtractFailure => "RUNTIME_EXTRACT_FAILURE",
            Self::PermissionFailure => "RUNTIME_PERMISSION_FAILURE",
            Self::PublishFailure => "RUNTIME_PUBLISH_FAILURE",
            Self::StagingFailure => "RUNTIME_STAGING_FAILURE",
        }
    }
}

pub struct RuntimeInstallError {
    code: RuntimeErrorCode,
    message: String,
    cleanup_safe: bool,
    http_status: Option<u16>,
}

impl RuntimeInstallError {
    fn new(code: RuntimeErrorCode, message: impl fmt::Display) -> Self {
        Self {
            code,
            message: message.to_string(),
            cleanup_safe: true,
            http_status: None,
        }
    }

    pub const fn code(&self) -> &'static str {
        self.code.as_str()
    }
}

impl fmt::Display for RuntimeInstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}

impl fmt::Debug for RuntimeInstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Result-returning main uses Debug; keep boot error codes grep-able there too.
        fmt::Display::fmt(self, f)
    }
}

impl std::error::Error for RuntimeInstallError {}

type Result<T> = std::result::Result<T, RuntimeInstallError>;

#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    pub libs_dir: PathBuf,
    pub plan_path: Option<PathBuf>,
    pub yt_dlp_version: String,
    pub python_release_tag: String,
    pub worker_yt_dlp_version: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePaths {
    pub binary_path: PathBuf,
    pub python_path: PathBuf,
    pub worker_path: PathBuf,
    pub python_path_entries: Vec<PathBuf>,
    pub runtime_id: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArtifactKind {
    Binary,
    Python,
    Wheel,
}

impl ArtifactKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Binary => "binary",
            Self::Python => "python",
            Self::Wheel => "wheel",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceChannel {
    GithubRelease,
    PythonBuildStandalone,
    PyPI,
}

impl SourceChannel {
    const fn as_str(self) -> &'static str {
        match self {
            Self::GithubRelease => "github-release",
            Self::PythonBuildStandalone => "python-build-standalone",
            Self::PyPI => "pypi",
        }
    }
}

#[derive(Clone, Debug)]
struct ResolvedArtifact {
    name: String,
    kind: ArtifactKind,
    source: SourceChannel,
    resolved_tag: String,
    url: String,
    sha256: String,
}

#[derive(Clone, Debug)]
struct RuntimePlan {
    yt_dlp_version: String,
    python_release_tag: String,
    python_version: String,
    artifacts: Vec<ResolvedArtifact>,
}

impl RuntimePlan {
    fn artifact(&self, kind: ArtifactKind) -> Result<&ResolvedArtifact> {
        self.artifacts
            .iter()
            .find(|artifact| artifact.kind == kind)
            .ok_or_else(|| {
                RuntimeInstallError::new(
                    RuntimeErrorCode::ResolveFailure,
                    format!("resolved runtime is missing {:?}", kind),
                )
            })
    }
}

pub async fn ensure_runtime(config: RuntimeConfig) -> Result<RuntimePaths> {
    let task = tokio::task::spawn_blocking(move || install_with_retry(config));
    match task.await {
        Ok(result) => result,
        Err(error) => Err(RuntimeInstallError::new(
            RuntimeErrorCode::StagingFailure,
            format!("runtime installation task failed: {error}"),
        )),
    }
}

fn install_with_retry(config: RuntimeConfig) -> Result<RuntimePaths> {
    let mut last_error = None;
    for attempt in 1..=DOWNLOAD_ATTEMPTS {
        match install_once(&config, WORKER_SCRIPT) {
            Ok(paths) => return Ok(paths),
            Err(mut error) => {
                error.message = format!("attempt={attempt}: {}", error.message);
                if error.code != RuntimeErrorCode::DownloadFailure || attempt == DOWNLOAD_ATTEMPTS {
                    return Err(error);
                }
                tracing::warn!(
                    attempt,
                    error = %error,
                    "runtime install attempt failed with a download error"
                );
                last_error = Some(error);
                sleep(Duration::from_secs(2u64.pow(attempt)));
            }
        }
    }
    match last_error {
        Some(error) => Err(error),
        None => Err(RuntimeInstallError::new(
            RuntimeErrorCode::DownloadFailure,
            "runtime installation failed without a captured error",
        )),
    }
}

fn install_once(config: &RuntimeConfig, worker_script: &[u8]) -> Result<RuntimePaths> {
    validate_config(config)?;
    let root = install_root(&config.libs_dir)?;
    let destination = root.join("runtime");
    let original = inspect_directory(&destination, &root)?;
    inspect_directory(&root.join("staging"), &root)?;
    let plan = resolve_plan(config)?;
    let worker_hash = sha256_hex(worker_script);
    let runtime_id = derive_runtime_id(&plan, &worker_hash)?;
    if original.is_some() {
        match validate_runtime(&destination, &plan, &runtime_id, &worker_hash) {
            Ok(paths) => {
                tracing::info!(runtime_id = %runtime_id, "runtime_reused");
                return Ok(paths);
            }
            Err(error) if error.code == RuntimeErrorCode::IntegrityFailure => {
                tracing::info!(runtime_id = %runtime_id, reason = %error, "runtime_reinstall_required");
            }
            Err(error) => return Err(error),
        }
    } else {
        tracing::info!(runtime_id = %runtime_id, reason = "absent", "runtime_reinstall_required");
    }

    let mut staging = OwnedStaging::create(&root)?;
    let result = prepare_staging(&staging.path, &plan, worker_script).and_then(|()| {
        write_manifest(&staging.path, &plan, &runtime_id, &worker_hash)?;
        sync_directory(&staging.path, "candidate-sync", "old-or-absent")?;
        validate_runtime(&staging.path, &plan, &runtime_id, &worker_hash)?;
        publish(
            &mut staging,
            &destination,
            original.as_ref(),
            &plan,
            &runtime_id,
            &worker_hash,
        )
    });
    match result {
        Ok(paths) => Ok(paths),
        Err(error) => {
            if !error.cleanup_safe {
                return Err(
                    staging.cleanup_error(&error, "writer quiescence is unknown; preserved")
                );
            }
            if let Err(cleanup) = staging.cleanup() {
                return Err(staging.cleanup_error(&error, &cleanup));
            }
            Err(error)
        }
    }
}

fn validate_config(config: &RuntimeConfig) -> Result<()> {
    if config.yt_dlp_version.trim().is_empty() {
        return Err(RuntimeInstallError::new(
            RuntimeErrorCode::ConfigInvalid,
            "runtime yt-dlp version must not be empty",
        ));
    }
    if !is_numeric_release_tag(&config.python_release_tag) {
        return Err(RuntimeInstallError::new(
            RuntimeErrorCode::ConfigInvalid,
            format!(
                "python release tag must be numeric: {}",
                config.python_release_tag
            ),
        ));
    }
    if let Some(worker_version) = config.worker_yt_dlp_version.as_deref() {
        if worker_version != config.yt_dlp_version {
            return Err(RuntimeInstallError::new(
                RuntimeErrorCode::VersionMismatch,
                format!(
                    "worker yt-dlp version {} does not match runtime version {}",
                    worker_version, config.yt_dlp_version
                ),
            ));
        }
    }
    Ok(())
}

fn resolve_plan(config: &RuntimeConfig) -> Result<RuntimePlan> {
    match config.plan_path.as_deref() {
        Some(path) => resolve_plan_file(path, config),
        None => resolve_live_plan(config),
    }
}

fn resolve_plan_file(path: &Path, config: &RuntimeConfig) -> Result<RuntimePlan> {
    let text = fs::read_to_string(path).map_err(|error| {
        RuntimeInstallError::new(
            RuntimeErrorCode::ConfigInvalid,
            format!("failed to read runtime plan {}: {error}", path.display()),
        )
    })?;
    let value: JsonValue = serde_json::from_str(&text).map_err(|error| {
        RuntimeInstallError::new(
            RuntimeErrorCode::ConfigInvalid,
            format!("invalid runtime plan {}: {error}", path.display()),
        )
    })?;
    if value.get("schema_version").and_then(JsonValue::as_i64) != Some(MANIFEST_SCHEMA_VERSION) {
        return Err(RuntimeInstallError::new(
            RuntimeErrorCode::ConfigInvalid,
            "runtime plan schema_version must be 1",
        ));
    }

    let yt_dlp_version = required_string(&value, "yt_dlp")?.to_owned();
    if yt_dlp_version != config.yt_dlp_version {
        return Err(RuntimeInstallError::new(
            RuntimeErrorCode::ConfigInvalid,
            format!(
                "runtime plan yt_dlp version {yt_dlp_version} does not match configured version {}",
                config.yt_dlp_version
            ),
        ));
    }
    let python = value.get("python").ok_or_else(|| {
        RuntimeInstallError::new(
            RuntimeErrorCode::ConfigInvalid,
            "runtime plan is missing python",
        )
    })?;
    let release_tag = required_string(python, "release_tag")?.to_owned();
    if release_tag != config.python_release_tag {
        return Err(RuntimeInstallError::new(
            RuntimeErrorCode::ConfigInvalid,
            format!(
                "runtime plan python release tag {release_tag} does not match configured tag {}",
                config.python_release_tag
            ),
        ));
    }
    let python_version = required_string(python, "python_version")?.to_owned();
    if !is_python_version(&python_version) {
        return Err(RuntimeInstallError::new(
            RuntimeErrorCode::ConfigInvalid,
            format!("invalid python version in runtime plan: {python_version}"),
        ));
    }
    let expected_binary = binary_asset_name().ok_or_else(|| {
        RuntimeInstallError::new(
            RuntimeErrorCode::ConfigInvalid,
            "unsupported binary platform",
        )
    })?;
    let target = pbs_target().ok_or_else(|| {
        RuntimeInstallError::new(
            RuntimeErrorCode::ConfigInvalid,
            "unsupported Python platform",
        )
    })?;
    let expected_python =
        format!("cpython-{python_version}+{release_tag}-{target}-install_only.tar.gz");
    let expected_wheel = format!("yt_dlp-{yt_dlp_version}-py3-none-any.whl");

    let values = value
        .get("artifacts")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| {
            RuntimeInstallError::new(
                RuntimeErrorCode::ConfigInvalid,
                "runtime plan must contain an artifacts array",
            )
        })?;
    let mut artifacts = Vec::with_capacity(values.len());
    for value in values {
        let name = required_string(value, "name")?;
        let url = required_string(value, "url")?;
        let sha256 = required_string(value, "sha256")?;
        if !is_sha256_hex(sha256) {
            return Err(RuntimeInstallError::new(
                RuntimeErrorCode::ConfigInvalid,
                format!("runtime plan artifact {name} has an invalid sha256"),
            ));
        }
        if !url.starts_with("https://") && !url.starts_with("file://") {
            return Err(RuntimeInstallError::new(
                RuntimeErrorCode::ConfigInvalid,
                format!("runtime plan artifact {name} has an unsupported URL scheme"),
            ));
        }
        let kind = artifact_kind(name).ok_or_else(|| {
            RuntimeInstallError::new(
                RuntimeErrorCode::ConfigInvalid,
                format!("runtime plan artifact is not supported on this platform: {name}"),
            )
        })?;
        let expected_name = match kind {
            ArtifactKind::Binary => expected_binary,
            ArtifactKind::Python => &expected_python,
            ArtifactKind::Wheel => &expected_wheel,
        };
        if name != expected_name {
            return Err(RuntimeInstallError::new(
                RuntimeErrorCode::ConfigInvalid,
                format!("runtime plan artifact {name} does not match {expected_name}"),
            ));
        }
        artifacts.push(make_plan_artifact(kind, name, url, sha256, config));
    }
    if artifacts.len() != 3
        || !artifacts
            .iter()
            .any(|artifact| artifact.kind == ArtifactKind::Binary)
        || !artifacts
            .iter()
            .any(|artifact| artifact.kind == ArtifactKind::Python)
        || !artifacts
            .iter()
            .any(|artifact| artifact.kind == ArtifactKind::Wheel)
    {
        return Err(RuntimeInstallError::new(
            RuntimeErrorCode::ConfigInvalid,
            "runtime plan must contain exactly one binary, python, and wheel artifact",
        ));
    }
    artifacts.sort_by_key(|artifact| match artifact.kind {
        ArtifactKind::Binary => 0,
        ArtifactKind::Python => 1,
        ArtifactKind::Wheel => 2,
    });

    Ok(RuntimePlan {
        yt_dlp_version,
        python_release_tag: release_tag,
        python_version,
        artifacts,
    })
}

fn make_plan_artifact(
    kind: ArtifactKind,
    name: &str,
    url: &str,
    sha256: &str,
    config: &RuntimeConfig,
) -> ResolvedArtifact {
    let resolved_tag = match kind {
        ArtifactKind::Binary | ArtifactKind::Wheel => config.yt_dlp_version.clone(),
        ArtifactKind::Python => config.python_release_tag.clone(),
    };
    let source = match kind {
        ArtifactKind::Binary => SourceChannel::GithubRelease,
        ArtifactKind::Python => SourceChannel::PythonBuildStandalone,
        ArtifactKind::Wheel => SourceChannel::PyPI,
    };
    ResolvedArtifact {
        name: name.to_owned(),
        kind,
        source,
        resolved_tag,
        url: url.to_owned(),
        sha256: sha256.to_ascii_lowercase(),
    }
}

fn resolve_live_plan(config: &RuntimeConfig) -> Result<RuntimePlan> {
    let binary_name = binary_asset_name().ok_or_else(|| {
        RuntimeInstallError::new(
            RuntimeErrorCode::ConfigInvalid,
            "unsupported platform for the yt-dlp binary artifact",
        )
    })?;
    let target = pbs_target().ok_or_else(|| {
        RuntimeInstallError::new(
            RuntimeErrorCode::ConfigInvalid,
            "unsupported platform for the python-build-standalone artifact",
        )
    })?;
    let python_version = resolve_live_python_version(&config.python_release_tag, target)?;
    let python_name = format!(
        "cpython-{}+{}-{}-install_only.tar.gz",
        python_version, config.python_release_tag, target
    );
    let wheel_name = format!("yt_dlp-{}-py3-none-any.whl", config.yt_dlp_version);

    let binary = github_release_artifact(
        "yt-dlp/yt-dlp",
        &config.yt_dlp_version,
        binary_name,
        ArtifactKind::Binary,
    )?;
    let python = github_release_artifact(
        "astral-sh/python-build-standalone",
        &config.python_release_tag,
        &python_name,
        ArtifactKind::Python,
    )?;
    let wheel = pypi_wheel_artifact(&config.yt_dlp_version, &wheel_name)?;

    Ok(RuntimePlan {
        yt_dlp_version: config.yt_dlp_version.clone(),
        python_release_tag: config.python_release_tag.clone(),
        python_version,
        artifacts: vec![binary, python, wheel],
    })
}

fn resolve_live_python_version(release_tag: &str, target: &str) -> Result<String> {
    let body = curl_to_memory(
        &format!(
            "https://api.github.com/repos/astral-sh/python-build-standalone/releases/tags/{release_tag}"
        ),
        64 * 1024 * 1024,
    )?;
    let value: JsonValue = parse_json(&body, "python-build-standalone release API")?;
    let suffix = format!("+{release_tag}-{target}-install_only.tar.gz");
    let versions: Vec<_> = value
        .get("assets")
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .filter_map(|asset| {
            let name = asset.get("name").and_then(JsonValue::as_str)?;
            let version = name.strip_prefix("cpython-")?.strip_suffix(&suffix)?;
            is_python_version(version).then_some(version)
        })
        .collect();
    let version = match versions.as_slice() {
        [version] => *version,
        _ => return Err(RuntimeInstallError::new(
            RuntimeErrorCode::ResolveFailure,
            format!("python-build-standalone release {release_tag} has {} CPython assets for {target}; expected one", versions.len()),
        )),
    };
    Ok(version.to_owned())
}

fn release_asset<'a>(assets: &'a [JsonValue], name: &str) -> Result<&'a JsonValue> {
    let mut matching = assets
        .iter()
        .filter(|asset| asset.get("name").and_then(JsonValue::as_str) == Some(name));
    let asset = matching.next().ok_or_else(|| {
        RuntimeInstallError::new(
            RuntimeErrorCode::ResolveFailure,
            format!("GitHub release has no asset {name}"),
        )
    })?;
    if matching.next().is_some() {
        return Err(RuntimeInstallError::new(
            RuntimeErrorCode::ResolveFailure,
            format!("GitHub release has ambiguous assets named {name}"),
        ));
    }
    Ok(asset)
}

fn github_release_artifact(
    repository: &str,
    release_tag: &str,
    asset_name: &str,
    kind: ArtifactKind,
) -> Result<ResolvedArtifact> {
    let body = curl_to_memory(
        &format!("https://api.github.com/repos/{repository}/releases/tags/{release_tag}"),
        64 * 1024 * 1024,
    )?;
    let value: JsonValue = parse_json(&body, "GitHub release API")?;
    let assets = value
        .get("assets")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| {
            RuntimeInstallError::new(
                RuntimeErrorCode::ResolveFailure,
                format!("GitHub release API response has no assets array: {repository}"),
            )
        })?;
    let asset = release_asset(assets, asset_name)?;
    let url = asset
        .get("browser_download_url")
        .and_then(metadata_url)
        .ok_or_else(|| {
            RuntimeInstallError::new(
                RuntimeErrorCode::ResolveFailure,
                format!("GitHub release API has no download URL for {asset_name}"),
            )
        })?;
    let sha256 = match github_asset_sha256(asset) {
        Some(digest) => digest,
        None if repository == "yt-dlp/yt-dlp" => {
            let sums_url = release_asset(assets, "SHA2-256SUMS")?
                .get("browser_download_url")
                .and_then(metadata_url)
                .ok_or_else(|| {
                    RuntimeInstallError::new(
                        RuntimeErrorCode::ResolveFailure,
                        format!("GitHub release {release_tag} has no SHA2-256SUMS asset"),
                    )
                })?;
            let sums = curl_to_memory(sums_url, 8 * 1024 * 1024).map_err(|mut error| {
                if error.code == RuntimeErrorCode::DownloadFailure && error.http_status == Some(404)
                {
                    error.code = RuntimeErrorCode::ResolveFailure;
                    error.message = format!("SHA2-256SUMS is missing: {}", error.message);
                }
                error
            })?;
            parse_sums_sha256(&sums, asset_name)?
        }
        None => {
            return Err(RuntimeInstallError::new(
                RuntimeErrorCode::ResolveFailure,
                format!("GitHub release API has no usable sha256 for {asset_name}"),
            ))
        }
    };

    Ok(ResolvedArtifact {
        name: asset_name.to_owned(),
        kind,
        source: match kind {
            ArtifactKind::Binary => SourceChannel::GithubRelease,
            ArtifactKind::Python => SourceChannel::PythonBuildStandalone,
            ArtifactKind::Wheel => SourceChannel::PyPI,
        },
        resolved_tag: release_tag.to_owned(),
        url: url.to_owned(),
        sha256,
    })
}

fn github_asset_sha256(asset: &JsonValue) -> Option<String> {
    asset
        .get("digest")
        .and_then(JsonValue::as_str)
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .filter(|digest| is_sha256_hex(digest))
        .map(str::to_ascii_lowercase)
}

fn parse_sums_sha256(body: &[u8], asset_name: &str) -> Result<String> {
    let text = std::str::from_utf8(body).map_err(|error| {
        RuntimeInstallError::new(
            RuntimeErrorCode::ResolveFailure,
            format!("SHA2-256SUMS is not UTF-8: {error}"),
        )
    })?;
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let Some(hash) = parts.next() else {
            continue;
        };
        let Some(name) = parts.next() else {
            continue;
        };
        if name == asset_name && is_sha256_hex(hash) {
            return Ok(hash.to_ascii_lowercase());
        }
    }
    Err(RuntimeInstallError::new(
        RuntimeErrorCode::ResolveFailure,
        format!("SHA2-256SUMS has no usable entry for {asset_name}"),
    ))
}

fn pypi_wheel_artifact(version: &str, expected_name: &str) -> Result<ResolvedArtifact> {
    let body = curl_to_memory(
        &format!("https://pypi.org/pypi/yt-dlp/{version}/json"),
        64 * 1024 * 1024,
    )?;
    let value: JsonValue = parse_json(&body, "PyPI JSON API")?;
    let wheels = value
        .pointer("/urls")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| {
            RuntimeInstallError::new(
                RuntimeErrorCode::ResolveFailure,
                "PyPI JSON API response has no urls array",
            )
        })?
        .iter()
        .filter(|item| {
            item.get("packagetype").and_then(JsonValue::as_str) == Some("bdist_wheel")
                && item
                    .get("filename")
                    .and_then(JsonValue::as_str)
                    .is_some_and(|name| name.ends_with("-py3-none-any.whl"))
        })
        .collect::<Vec<_>>();
    if wheels.len() != 1 {
        return Err(RuntimeInstallError::new(
            RuntimeErrorCode::ResolveFailure,
            format!(
                "PyPI has {} py3-none-any wheels for yt-dlp {version}",
                wheels.len()
            ),
        ));
    }
    let wheel = wheels[0];
    let name = wheel
        .get("filename")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| {
            RuntimeInstallError::new(
                RuntimeErrorCode::ResolveFailure,
                "PyPI JSON API response has no wheel filename",
            )
        })?;
    let url = wheel.get("url").and_then(metadata_url).ok_or_else(|| {
        RuntimeInstallError::new(
            RuntimeErrorCode::ResolveFailure,
            "PyPI JSON API response has no wheel URL",
        )
    })?;
    let sha256 = wheel
        .pointer("/digests/sha256")
        .and_then(JsonValue::as_str)
        .filter(|digest| is_sha256_hex(digest))
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| {
            RuntimeInstallError::new(
                RuntimeErrorCode::ResolveFailure,
                "PyPI JSON API response has no usable wheel sha256",
            )
        })?;
    if name != expected_name {
        return Err(RuntimeInstallError::new(
            RuntimeErrorCode::ResolveFailure,
            format!("PyPI wheel {name} does not match expected artifact {expected_name}"),
        ));
    }

    Ok(ResolvedArtifact {
        name: name.to_owned(),
        kind: ArtifactKind::Wheel,
        source: SourceChannel::PyPI,
        resolved_tag: version.to_owned(),
        url: url.to_owned(),
        sha256,
    })
}

fn derive_runtime_id(plan: &RuntimePlan, worker_hash: &str) -> Result<String> {
    let identity = json!({
        "schema_version": MANIFEST_SCHEMA_VERSION,
        "worker_script_sha256": worker_hash,
        "artifacts": plan
            .artifacts
            .iter()
            .map(artifact_identity_json)
            .collect::<Vec<_>>(),
    });
    let bytes = serde_json::to_vec(&identity).map_err(|error| {
        RuntimeInstallError::new(
            RuntimeErrorCode::ConfigInvalid,
            format!("failed to serialize runtime identity: {error}"),
        )
    })?;
    let digest = sha256_hex(&bytes);
    Ok(format!(
        "yt-dlp-{}-py{}-pbs{}-{}",
        normalize_yt_dlp_version(&plan.yt_dlp_version),
        plan.python_version,
        plan.python_release_tag,
        &digest[..8]
    ))
}

fn artifact_identity_json(artifact: &ResolvedArtifact) -> JsonValue {
    json!({
        "name": artifact.name,
        "kind": artifact.kind.as_str(),
        "source": artifact.source.as_str(),
        "resolved_tag": artifact.resolved_tag,
        "sha256": artifact.sha256,
    })
}

fn normalize_yt_dlp_version(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn validate_runtime(
    directory: &Path,
    plan: &RuntimePlan,
    runtime_id: &str,
    worker_hash: &str,
) -> Result<RuntimePaths> {
    let manifest = read_manifest(directory)?;
    verify_manifest(&manifest, plan, runtime_id, worker_hash)?;
    for artifact in &plan.artifacts {
        let path = directory.join(ARTIFACTS_DIR).join(&artifact.name);
        verify_file(&path, &artifact.sha256)?;
    }
    let worker_path = directory.join("worker").join(WORKER_SCRIPT_NAME);
    verify_file(&worker_path, worker_hash)?;
    let binary_name = installed_binary_name();
    verify_file(
        &directory.join("bin").join(binary_name),
        &plan.artifact(ArtifactKind::Binary)?.sha256,
    )?;
    let paths = runtime_paths(directory, runtime_id, binary_name);
    verify_required_paths(directory, &paths)?;
    Ok(paths)
}

fn verify_required_paths(directory: &Path, paths: &RuntimePaths) -> Result<()> {
    for path in [
        paths.python_path.clone(),
        paths.python_path_entries[0]
            .join("yt_dlp")
            .join("__init__.py"),
        paths.python_path_entries[0]
            .join("yt_dlp")
            .join("version.py"),
    ] {
        let resolved = fs::canonicalize(&path).map_err(|error| {
            RuntimeInstallError::new(
                RuntimeErrorCode::IntegrityFailure,
                format!("required runtime path {}: {error}", path.display()),
            )
        })?;
        if !resolved.starts_with(directory) || !resolved.is_file() {
            return Err(RuntimeInstallError::new(
                RuntimeErrorCode::IntegrityFailure,
                format!(
                    "required runtime file is missing or escapes installation: {}",
                    path.display()
                ),
            ));
        }
    }
    Ok(())
}

fn read_manifest(directory: &Path) -> Result<JsonValue> {
    let path = directory.join(MANIFEST_NAME);
    let text = fs::read_to_string(&path).map_err(|error| {
        RuntimeInstallError::new(
            RuntimeErrorCode::IntegrityFailure,
            format!(
                "failed to read runtime manifest {}: {error}",
                path.display()
            ),
        )
    })?;
    serde_json::from_str(&text).map_err(|error| {
        RuntimeInstallError::new(
            RuntimeErrorCode::IntegrityFailure,
            format!("invalid runtime manifest {}: {error}", path.display()),
        )
    })
}

fn verify_manifest(
    manifest: &JsonValue,
    plan: &RuntimePlan,
    runtime_id: &str,
    worker_hash: &str,
) -> Result<()> {
    if manifest.get("schema_version").and_then(JsonValue::as_i64) != Some(MANIFEST_SCHEMA_VERSION) {
        return Err(RuntimeInstallError::new(
            RuntimeErrorCode::IntegrityFailure,
            "runtime manifest schema_version is not 1",
        ));
    }
    if manifest.get("runtime_id").and_then(JsonValue::as_str) != Some(runtime_id) {
        return Err(RuntimeInstallError::new(
            RuntimeErrorCode::IntegrityFailure,
            "runtime manifest runtime_id does not match the requested identity",
        ));
    }
    let expected_platform = json!({
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
    });
    if manifest.get("platform") != Some(&expected_platform) {
        return Err(RuntimeInstallError::new(
            RuntimeErrorCode::IntegrityFailure,
            "runtime manifest platform does not match this host",
        ));
    }
    let expected_versions = json!({
        "yt_dlp": plan.yt_dlp_version,
        "python": plan.python_version,
        "python_release_tag": plan.python_release_tag,
    });
    if manifest.get("versions") != Some(&expected_versions) {
        return Err(RuntimeInstallError::new(
            RuntimeErrorCode::IntegrityFailure,
            "runtime manifest versions do not match the requested runtime",
        ));
    }
    if manifest
        .get("worker_script_sha256")
        .and_then(JsonValue::as_str)
        != Some(worker_hash)
    {
        return Err(RuntimeInstallError::new(
            RuntimeErrorCode::IntegrityFailure,
            "runtime manifest worker script hash does not match this build",
        ));
    }
    let artifacts = manifest
        .get("artifacts")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| {
            RuntimeInstallError::new(
                RuntimeErrorCode::IntegrityFailure,
                "runtime manifest has no artifacts array",
            )
        })?;
    if artifacts.len() != plan.artifacts.len() {
        return Err(RuntimeInstallError::new(
            RuntimeErrorCode::IntegrityFailure,
            "runtime manifest artifact count does not match the requested runtime",
        ));
    }
    for artifact in &plan.artifacts {
        let entry = artifacts
            .iter()
            .find(|entry| {
                entry.get("name").and_then(JsonValue::as_str) == Some(artifact.name.as_str())
            })
            .ok_or_else(|| {
                RuntimeInstallError::new(
                    RuntimeErrorCode::IntegrityFailure,
                    format!("runtime manifest has no artifact {}", artifact.name),
                )
            })?;
        let expected = artifact_identity_json(artifact);
        if !artifact_entry_matches(entry, &expected) {
            return Err(RuntimeInstallError::new(
                RuntimeErrorCode::IntegrityFailure,
                format!("runtime manifest artifact {} does not match", artifact.name),
            ));
        }
    }
    Ok(())
}

fn artifact_entry_matches(entry: &JsonValue, expected: &JsonValue) -> bool {
    ["name", "kind", "source", "resolved_tag", "sha256"]
        .iter()
        .all(|key| entry.get(key) == expected.get(key))
}

fn runtime_paths(directory: &Path, runtime_id: &str, binary_name: &str) -> RuntimePaths {
    let python_path = if cfg!(windows) {
        directory.join("python").join("python.exe")
    } else {
        directory.join("python").join("bin").join("python3")
    };
    RuntimePaths {
        binary_path: directory.join("bin").join(binary_name),
        python_path,
        worker_path: directory.join("worker").join(WORKER_SCRIPT_NAME),
        python_path_entries: vec![directory.join("python").join("site-packages")],
        runtime_id: runtime_id.to_owned(),
    }
}

#[derive(Debug, PartialEq)]
struct DirectoryIdentity {
    created: Option<std::time::SystemTime>,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

#[derive(Debug, PartialEq)]
struct DirectoryState {
    identity: DirectoryIdentity,
    modified: Option<std::time::SystemTime>,
}

fn directory_state(metadata: &fs::Metadata) -> DirectoryState {
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;
    DirectoryState {
        identity: DirectoryIdentity {
            created: metadata.created().ok(),
            #[cfg(unix)]
            device: metadata.dev(),
            #[cfg(unix)]
            inode: metadata.ino(),
        },
        modified: metadata.modified().ok(),
    }
}

fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0 // FILE_ATTRIBUTE_REPARSE_POINT
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

fn same_filesystem(metadata: &fs::Metadata, parent: &Path) -> std::io::Result<bool> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(metadata.dev() == fs::metadata(parent)?.dev())
    }
    #[cfg(not(unix))]
    {
        // Windows volume mount points are rejected as reparse points.
        let _ = (metadata, parent);
        Ok(true)
    }
}

fn publish_error(phase: &str, state: &str, detail: impl fmt::Display) -> RuntimeInstallError {
    RuntimeInstallError::new(
        RuntimeErrorCode::PublishFailure,
        format!("phase={phase} runtime_state={state}: {detail}"),
    )
}

fn install_root(path: &Path) -> Result<PathBuf> {
    fs::create_dir_all(path).map_err(|error| publish_error("root", "unknown", error))?;
    let root = fs::canonicalize(path).map_err(|error| publish_error("root", "unknown", error))?;
    let metadata =
        fs::symlink_metadata(&root).map_err(|error| publish_error("root", "unknown", error))?;
    if !metadata.is_dir() || is_link(&metadata) {
        return Err(publish_error(
            "root",
            "unknown",
            "libs-dir is not an ordinary directory",
        ));
    }
    Ok(root)
}

fn inspect_directory(path: &Path, parent: &Path) -> Result<Option<DirectoryState>> {
    if path.parent() != Some(parent) {
        return Err(publish_error(
            "layout",
            "unknown",
            "directory escapes its expected parent",
        ));
    }
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(publish_error("layout", "unknown", error)),
    };
    if !metadata.is_dir()
        || is_link(&metadata)
        || !same_filesystem(&metadata, parent)
            .map_err(|error| publish_error("layout", "unknown", error))?
        || fs::canonicalize(path).map_err(|error| publish_error("layout", "unknown", error))?
            != path
    {
        return Err(publish_error(
            "layout",
            "unknown",
            format!("unsafe directory {}", path.display()),
        ));
    }
    Ok(Some(directory_state(&metadata)))
}

// This guard is an invocation-local creation record, not a cross-boot marker.
// No Drop deletion: panic/uncertain child liveness must preserve the work.
struct OwnedStaging {
    root: PathBuf,
    root_identity: DirectoryIdentity,
    parent_identity: DirectoryIdentity,
    path: PathBuf,
    identity: DirectoryIdentity,
    published: bool,
}

impl OwnedStaging {
    fn create(root: &Path) -> Result<Self> {
        let parent = root.join("staging");
        let result = (|| {
            if inspect_directory(&parent, root)?.is_none() {
                fs::create_dir(&parent)
                    .map_err(|error| publish_error("staging-create", "old-or-absent", error))?;
            }
            let parent_state = inspect_directory(&parent, root)?.ok_or_else(|| {
                publish_error("staging-create", "unknown", "staging root disappeared")
            })?;
            let root_identity = directory_state(
                &fs::symlink_metadata(root)
                    .map_err(|error| publish_error("staging-create", "unknown", error))?,
            )
            .identity;
            let path = tempfile::Builder::new()
                .prefix("runtime-")
                .tempdir_in(&parent)
                .map_err(|error| publish_error("staging-create", "old-or-absent", error))?
                .keep();
            let identity = inspect_directory(&path, &parent)?
                .ok_or_else(|| {
                    publish_error(
                        "staging-create",
                        "unknown",
                        format!("created staging disappeared: {}", path.display()),
                    )
                })?
                .identity;
            Ok(Self {
                root: root.to_owned(),
                root_identity,
                parent_identity: parent_state.identity,
                path,
                identity,
                published: false,
            })
        })();
        result.map_err(|error: RuntimeInstallError| {
            RuntimeInstallError::new(RuntimeErrorCode::StagingFailure, error)
        })
    }

    fn recheck(&self) -> Result<()> {
        let metadata = fs::symlink_metadata(&self.root)
            .map_err(|error| publish_error("ownership", "unknown", error))?;
        if is_link(&metadata) || directory_state(&metadata).identity != self.root_identity {
            return Err(publish_error("ownership", "unknown", "libs root changed"));
        }
        let parent = self.root.join("staging");
        if inspect_directory(&parent, &self.root)?
            .map(|state| state.identity)
            .as_ref()
            != Some(&self.parent_identity)
            || inspect_directory(&self.path, &parent)?
                .map(|state| state.identity)
                .as_ref()
                != Some(&self.identity)
        {
            return Err(publish_error(
                "ownership",
                "unknown",
                "staging ownership/containment changed",
            ));
        }
        Ok(())
    }

    fn cleanup(&self) -> Result<()> {
        if self.published {
            return Ok(());
        }
        self.recheck()?;
        #[cfg(test)]
        tests::io_fault("cleanup")
            .map_err(|error| RuntimeInstallError::new(RuntimeErrorCode::StagingFailure, error))?;
        remove_tree_entry(&self.path, &self.root.join("staging"))
            .map_err(|error| RuntimeInstallError::new(RuntimeErrorCode::StagingFailure, error))
    }

    fn cleanup_error(
        &self,
        primary: &RuntimeInstallError,
        detail: impl fmt::Display,
    ) -> RuntimeInstallError {
        let error = RuntimeInstallError::new(
            RuntimeErrorCode::StagingFailure,
            format!(
                "phase=cleanup path={} primary=[{primary}]: {detail}",
                self.path.display()
            ),
        );
        tracing::error!(error = %error, "runtime staging cleanup failed");
        error
    }
}

fn remove_tree_entry(path: &Path, parent: &Path) -> std::io::Result<()> {
    if path.parent() != Some(parent) {
        return Err(std::io::Error::other("removal escapes expected parent"));
    }
    let metadata = fs::symlink_metadata(path)?;
    if is_link(&metadata) {
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x10 != 0 {
                return fs::remove_dir(path);
            }
        }
        return fs::remove_file(path);
    }
    if metadata.is_dir() {
        if !same_filesystem(&metadata, parent)? || fs::canonicalize(path)? != path {
            return Err(std::io::Error::other("unsafe directory during removal"));
        }
        for entry in fs::read_dir(path)? {
            remove_tree_entry(&entry?.path(), path)?;
        }
        fs::remove_dir(path)
    } else {
        fs::remove_file(path)
    }
}

fn prepare_staging(staging: &Path, plan: &RuntimePlan, worker_script: &[u8]) -> Result<()> {
    let artifacts_dir = staging.join(ARTIFACTS_DIR);
    fs::create_dir_all(&artifacts_dir).map_err(|error| {
        RuntimeInstallError::new(
            RuntimeErrorCode::StagingFailure,
            format!("failed to create artifacts staging directory: {error}"),
        )
    })?;

    for artifact in &plan.artifacts {
        let destination = artifacts_dir.join(&artifact.name);
        fetch_to_file(artifact, &destination)?;
        verify_file(&destination, &artifact.sha256)?;
    }

    let binary = plan.artifact(ArtifactKind::Binary)?;
    let binary_name = installed_binary_name();
    let bin_dir = staging.join("bin");
    fs::create_dir_all(&bin_dir).map_err(|error| {
        RuntimeInstallError::new(
            RuntimeErrorCode::StagingFailure,
            format!("failed to create bin staging directory: {error}"),
        )
    })?;
    let binary_path = bin_dir.join(binary_name);
    fs::copy(artifacts_dir.join(&binary.name), &binary_path).map_err(|error| {
        RuntimeInstallError::new(
            RuntimeErrorCode::ExtractFailure,
            format!("failed to install yt-dlp binary: {error}"),
        )
    })?;
    verify_file(&binary_path, &binary.sha256)?;

    let python = plan.artifact(ArtifactKind::Python)?;
    extract_tar(&artifacts_dir.join(&python.name), staging)?;

    let wheel = plan.artifact(ArtifactKind::Wheel)?;
    let site_packages = staging.join("python").join("site-packages");
    fs::create_dir_all(&site_packages).map_err(|error| {
        RuntimeInstallError::new(
            RuntimeErrorCode::StagingFailure,
            format!("failed to create site-packages: {error}"),
        )
    })?;
    extract_zip(&artifacts_dir.join(&wheel.name), &site_packages)?;

    let worker_dir = staging.join("worker");
    fs::create_dir_all(&worker_dir).map_err(|error| {
        RuntimeInstallError::new(
            RuntimeErrorCode::StagingFailure,
            format!("failed to create worker staging directory: {error}"),
        )
    })?;
    let worker_path = worker_dir.join(WORKER_SCRIPT_NAME);
    fs::write(&worker_path, worker_script).map_err(|error| {
        RuntimeInstallError::new(
            RuntimeErrorCode::StagingFailure,
            format!("failed to copy worker script: {error}"),
        )
    })?;
    verify_file(&worker_path, &sha256_hex(worker_script))?;

    make_binaries_executable(staging, binary_name)?;
    verify_required_paths(staging, &runtime_paths(staging, "", binary_name))?;
    Ok(())
}

fn fetch_to_file(artifact: &ResolvedArtifact, destination: &Path) -> Result<()> {
    let source = source_path(&artifact.url)?;
    match source {
        Source::Local(path) => {
            fs::copy(&path, destination).map_err(|error| {
                RuntimeInstallError::new(
                    RuntimeErrorCode::DownloadFailure,
                    format!("failed to copy artifact {}: {error}", artifact.name),
                )
            })?;
        }
        Source::Url(url) => {
            curl_to_file(&url, destination).map_err(|mut error| {
                error.message = format!("artifact={}: {}", artifact.name, error.message);
                error
            })?;
        }
    }
    let metadata = fs::metadata(destination).map_err(|error| {
        RuntimeInstallError::new(
            RuntimeErrorCode::DownloadFailure,
            format!("failed to stat artifact {}: {error}", artifact.name),
        )
    })?;
    if metadata.len() > DEFAULT_MAX_ARTIFACT_BYTES {
        return Err(RuntimeInstallError::new(
            RuntimeErrorCode::DownloadFailure,
            format!("artifact {} exceeds the size limit", artifact.name),
        ));
    }
    Ok(())
}

enum Source {
    Local(PathBuf),
    Url(String),
}

fn source_path(url: &str) -> Result<Source> {
    if let Some(rest) = url.strip_prefix("file://") {
        let rest = if cfg!(windows) {
            rest.strip_prefix('/').unwrap_or(rest)
        } else {
            rest
        };
        return Ok(Source::Local(PathBuf::from(rest)));
    }
    Ok(Source::Url(url.to_owned()))
}

fn curl_to_file(url: &str, destination: &Path) -> Result<()> {
    let program = if cfg!(windows) { "curl.exe" } else { "curl" };
    let mut command = Command::new(program);
    command
        .args([
            "-fsSL",
            "--retry",
            "0",
            "--max-time",
            &DEFAULT_DOWNLOAD_TIMEOUT.as_secs().to_string(),
            "--max-filesize",
            &DEFAULT_MAX_ARTIFACT_BYTES.to_string(),
            "-o",
        ])
        .arg(destination)
        .arg(url);
    run_command(
        command,
        RuntimeErrorCode::CurlMissing,
        RuntimeErrorCode::DownloadFailure,
    )?;
    Ok(())
}

fn curl_to_memory(url: &str, max_bytes: u64) -> Result<Vec<u8>> {
    let program = if cfg!(windows) { "curl.exe" } else { "curl" };
    let mut command = Command::new(program);
    command
        .args([
            "-fsSL",
            "--retry",
            "0",
            "--max-time",
            &DEFAULT_DOWNLOAD_TIMEOUT.as_secs().to_string(),
            "--max-filesize",
            &max_bytes.to_string(),
            "--write-out",
            "\n%{http_code}",
        ])
        .arg(url);
    let basename = url
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .split_once("://")
        .and_then(|(_, host_path)| host_path.split_once('/'))
        .map(|(_, path)| path.rsplit('/').next().unwrap_or("metadata"))
        .unwrap_or("metadata");
    let mut output = command_output(
        command,
        RuntimeErrorCode::CurlMissing,
        RuntimeErrorCode::DownloadFailure,
    )
    .map_err(|mut error| {
        error.message = format!("source={basename}: {}", error.message);
        error
    })?;
    // curl appends the final response status even with --fail. Only SUMS 404
    // is resolution exhaustion; other HTTP/transport errors remain retryable.
    let http_status = output
        .stdout
        .iter()
        .rposition(|byte| *byte == b'\n')
        .and_then(|index| {
            let status = std::str::from_utf8(&output.stdout[index + 1..])
                .ok()?
                .parse()
                .ok()?;
            output.stdout.truncate(index);
            Some(status)
        });
    if !output.status.success() {
        let mut error = command_failure(program, output.status, RuntimeErrorCode::DownloadFailure);
        error.http_status = http_status;
        error.message = format!(
            "source={basename}: {} http_status={http_status:?}",
            error.message
        );
        return Err(error);
    }
    if output.stdout.len() as u64 > max_bytes {
        return Err(RuntimeInstallError::new(
            RuntimeErrorCode::DownloadFailure,
            format!("source={basename}: metadata exceeds size limit {max_bytes}"),
        ));
    }
    Ok(output.stdout)
}

fn run_command(
    command: Command,
    missing_code: RuntimeErrorCode,
    failure_code: RuntimeErrorCode,
) -> Result<std::process::Output> {
    let program = command.get_program().to_string_lossy().into_owned();
    let output = command_output(command, missing_code, failure_code)?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(command_failure(&program, output.status, failure_code))
    }
}

fn command_failure(
    program: &str,
    status: std::process::ExitStatus,
    code: RuntimeErrorCode,
) -> RuntimeInstallError {
    RuntimeInstallError::new(
        code,
        format!(
            "tool={} failed with exit status {:?}",
            program.trim_end_matches(".exe"),
            status.code()
        ),
    )
}

fn command_output(
    mut command: Command,
    missing_code: RuntimeErrorCode,
    failure_code: RuntimeErrorCode,
) -> Result<std::process::Output> {
    let program = command.get_program().to_string_lossy().into_owned();
    #[cfg(test)]
    if let Some(output) = tests::command_reply(&mut command) {
        return Ok(output);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let child = command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|error| {
            let code = if error.kind() == std::io::ErrorKind::NotFound {
                missing_code
            } else {
                failure_code
            };
            RuntimeInstallError::new(
                code,
                format!(
                    "tool={} failed to spawn: {error}",
                    program.trim_end_matches(".exe")
                ),
            )
        })?;
    // All successful collections await the child and its output reader. If
    // collection fails, liveness is uncertain: preserve staging, never retry.
    let output = child.wait_with_output();
    #[cfg(test)]
    let output = output.and_then(|output| tests::io_fault("child-wait").map(|()| output));
    let output = output.map_err(|error| {
        let mut error = RuntimeInstallError::new(
            RuntimeErrorCode::StagingFailure,
            format!("failed to await {program}; quiescence unknown: {error}"),
        );
        error.cleanup_safe = false;
        error
    })?;
    Ok(output)
}

fn parse_json(bytes: &[u8], source: &'static str) -> Result<JsonValue> {
    serde_json::from_slice(bytes).map_err(|error| {
        RuntimeInstallError::new(
            RuntimeErrorCode::ResolveFailure,
            format!("invalid JSON response from {source}: {error}"),
        )
    })
}

fn extract_tar(archive: &Path, destination: &Path) -> Result<()> {
    let program = if cfg!(windows) { "tar.exe" } else { "tar" };
    let mut command = Command::new(program);
    command
        .args(["-xzf"])
        .arg(archive)
        .arg("-C")
        .arg(destination);
    if cfg!(unix) {
        command.arg("--no-same-owner");
    }
    run_command(
        command,
        RuntimeErrorCode::CurlMissing,
        RuntimeErrorCode::ExtractFailure,
    )?;
    Ok(())
}

fn extract_zip(archive: &Path, destination: &Path) -> Result<()> {
    let file = File::open(archive).map_err(|error| {
        RuntimeInstallError::new(
            RuntimeErrorCode::ExtractFailure,
            format!("failed to open wheel {}: {error}", archive.display()),
        )
    })?;
    let mut archive = zip::ZipArchive::new(file).map_err(|error| {
        RuntimeInstallError::new(
            RuntimeErrorCode::ExtractFailure,
            format!("failed to read wheel {}: {error}", archive.display()),
        )
    })?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| {
            RuntimeInstallError::new(
                RuntimeErrorCode::ExtractFailure,
                format!("failed to read wheel entry: {error}"),
            )
        })?;
        let Some(relative) = entry.enclosed_name() else {
            return Err(RuntimeInstallError::new(
                RuntimeErrorCode::ExtractFailure,
                format!("unsafe wheel entry path: {}", entry.name()),
            ));
        };
        let output_path = destination.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&output_path).map_err(|error| {
                RuntimeInstallError::new(
                    RuntimeErrorCode::ExtractFailure,
                    format!("failed to create wheel directory: {error}"),
                )
            })?;
            continue;
        }
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                RuntimeInstallError::new(
                    RuntimeErrorCode::ExtractFailure,
                    format!("failed to create wheel parent directory: {error}"),
                )
            })?;
        }
        let mut output = File::create(&output_path).map_err(|error| {
            RuntimeInstallError::new(
                RuntimeErrorCode::ExtractFailure,
                format!(
                    "failed to create wheel entry {}: {error}",
                    output_path.display()
                ),
            )
        })?;
        std::io::copy(&mut entry, &mut output).map_err(|error| {
            RuntimeInstallError::new(
                RuntimeErrorCode::ExtractFailure,
                format!(
                    "failed to extract wheel entry {}: {error}",
                    output_path.display()
                ),
            )
        })?;
    }
    Ok(())
}

fn make_binaries_executable(staging: &Path, binary_name: &str) -> Result<()> {
    if cfg!(windows) {
        return Ok(());
    }
    let binary = staging.join("bin").join(binary_name);
    if binary.is_file() {
        set_executable(&binary)?;
    }
    let python_bin = staging.join("python").join("bin");
    if python_bin.is_dir() {
        for entry in fs::read_dir(&python_bin).map_err(|error| {
            RuntimeInstallError::new(
                RuntimeErrorCode::PermissionFailure,
                format!("failed to read python bin directory: {error}"),
            )
        })? {
            let entry = entry.map_err(|error| {
                RuntimeInstallError::new(
                    RuntimeErrorCode::PermissionFailure,
                    format!("failed to read python bin entry: {error}"),
                )
            })?;
            set_executable(&entry.path())?;
        }
    }
    Ok(())
}

fn set_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .map_err(|error| {
                RuntimeInstallError::new(
                    RuntimeErrorCode::PermissionFailure,
                    format!("failed to read permissions for {}: {error}", path.display()),
                )
            })?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).map_err(|error| {
            RuntimeInstallError::new(
                RuntimeErrorCode::PermissionFailure,
                format!("failed to set permissions for {}: {error}", path.display()),
            )
        })?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn publish(
    staging: &mut OwnedStaging,
    destination: &Path,
    original: Option<&DirectoryState>,
    plan: &RuntimePlan,
    runtime_id: &str,
    worker_hash: &str,
) -> Result<RuntimePaths> {
    staging.recheck()?;
    if inspect_directory(destination, &staging.root)?.as_ref() != original {
        return Err(publish_error(
            "pre-retirement",
            "unknown",
            "runtime changed during preparation",
        ));
    }
    if original.is_some() {
        match fs::remove_file(destination.join(MANIFEST_NAME)) {
            Ok(()) => (),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(publish_error("invalidate-manifest", "old", error)),
        }
        sync_directory(destination, "invalidate-sync", "invalid")?;
        #[cfg(test)]
        tests::io_fault("remove-retired")
            .map_err(|error| publish_error("remove-retired", "invalid", error))?;
        remove_tree_entry(destination, &staging.root)
            .map_err(|error| publish_error("remove-retired", "invalid", error))?;
    }
    if inspect_directory(destination, &staging.root)?.is_some() {
        return Err(publish_error(
            "rename",
            "unknown",
            "runtime target is no longer absent",
        ));
    }
    #[cfg(test)]
    tests::io_fault("rename").map_err(|error| publish_error("rename", "absent", error))?;
    fs::rename(&staging.path, destination)
        .map_err(|error| publish_error("rename", "unknown", error))?;
    // The candidate is now the installation, even if sync or validation fails.
    staging.published = true;
    sync_directory(&staging.root, "root-sync", "candidate")?;
    #[cfg(test)]
    tests::after_rename(destination);
    validate_runtime(destination, plan, runtime_id, worker_hash).map_err(|mut error| {
        error.message = format!(
            "phase=post-publication runtime_state=candidate: {}",
            error.message
        );
        error
    })
}

fn write_manifest(
    staging: &Path,
    plan: &RuntimePlan,
    runtime_id: &str,
    worker_hash: &str,
) -> Result<()> {
    let artifacts = plan
        .artifacts
        .iter()
        .map(|artifact| -> Result<JsonValue> {
            let size = fs::metadata(staging.join(ARTIFACTS_DIR).join(&artifact.name))
                .map(|metadata| metadata.len())
                .map_err(|error| publish_error("candidate-manifest", "old-or-absent", error))?;
            Ok(json!({
                "name": artifact.name,
                "kind": artifact.kind.as_str(),
                "source": artifact.source.as_str(),
                "resolved_tag": artifact.resolved_tag,
                "url": artifact.url,
                "sha256": artifact.sha256,
                "size_bytes": size,
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    let manifest = json!({
        "schema_version": MANIFEST_SCHEMA_VERSION,
        "runtime_id": runtime_id,
        "platform": { "os": std::env::consts::OS, "arch": std::env::consts::ARCH },
        "versions": {
            "yt_dlp": plan.yt_dlp_version,
            "python": plan.python_version,
            "python_release_tag": plan.python_release_tag,
        },
        "worker_script_sha256": worker_hash,
        "artifacts": artifacts,
    });
    let bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| publish_error("candidate-manifest", "old-or-absent", error))?;
    let path = staging.join(MANIFEST_NAME);
    let mut file = File::create(&path)
        .map_err(|error| publish_error("candidate-manifest", "old-or-absent", error))?;
    file.write_all(&bytes)
        .and_then(|()| {
            #[cfg(test)]
            tests::io_fault("candidate-manifest-sync")?;
            file.sync_all()
        })
        .map_err(|error| publish_error("candidate-manifest-sync", "old-or-absent", error))?;
    Ok(())
}

fn sync_directory(path: &Path, phase: &str, state: &str) -> Result<()> {
    #[cfg(unix)]
    {
        #[cfg(test)]
        tests::io_fault(phase).map_err(|error| publish_error(phase, state, error))?;
        File::open(path)
            .and_then(|file| file.sync_all())
            .map_err(|error| publish_error(phase, state, error))?;
    }
    #[cfg(not(unix))]
    let _ = (path, phase, state);
    Ok(())
}

fn required_string<'a>(value: &'a JsonValue, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(JsonValue::as_str)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| {
            RuntimeInstallError::new(
                RuntimeErrorCode::ConfigInvalid,
                format!("missing or empty JSON string field: {key}"),
            )
        })
}

fn metadata_url(value: &JsonValue) -> Option<&str> {
    value.as_str().filter(|url| {
        url.strip_prefix("https://")
            .is_some_and(|host_path| !host_path.is_empty() && !host_path.starts_with('/'))
    })
}

fn is_numeric_release_tag(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_python_version(value: &str) -> bool {
    let mut components = value.split('.');
    let mut count = 0;
    for component in components.by_ref() {
        if component.is_empty() || !component.bytes().all(|byte| byte.is_ascii_digit()) {
            return false;
        }
        count += 1;
    }
    count == 3
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value.bytes().all(|byte| {
            byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte) || (b'A'..=b'F').contains(&byte)
        })
}

fn artifact_kind(name: &str) -> Option<ArtifactKind> {
    if name == "yt-dlp.exe"
        || name == "yt-dlp_x86.exe"
        || name == "yt-dlp_linux"
        || name == "yt-dlp_linux_armv7l"
        || name == "yt-dlp_linux_aarch64"
        || name == "yt-dlp_macos"
    {
        return Some(ArtifactKind::Binary);
    }
    if name.starts_with("cpython-") && name.ends_with("-install_only.tar.gz") {
        return Some(ArtifactKind::Python);
    }
    if name.starts_with("yt_dlp-") && name.ends_with("-py3-none-any.whl") {
        return Some(ArtifactKind::Wheel);
    }
    None
}

fn binary_asset_name() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86") => Some("yt-dlp_x86.exe"),
        ("windows", "x86_64") => Some("yt-dlp.exe"),
        ("linux", "arm") => Some("yt-dlp_linux_armv7l"),
        ("linux", "aarch64") => Some("yt-dlp_linux_aarch64"),
        ("linux", "x86_64") => Some("yt-dlp_linux"),
        ("macos", "aarch64") | ("macos", "x86_64") => Some("yt-dlp_macos"),
        _ => None,
    }
}

fn installed_binary_name() -> &'static str {
    if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    }
}

fn pbs_target() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86") => Some("i686-pc-windows-msvc"),
        ("windows", "x86_64") => Some("x86_64-pc-windows-msvc"),
        ("linux", "arm") => Some("armv7-unknown-linux-gnueabihf"),
        ("linux", "aarch64") => Some("aarch64-unknown-linux-gnu"),
        ("linux", "x86_64") => Some("x86_64-unknown-linux-gnu"),
        ("macos", "aarch64") => Some("aarch64-apple-darwin"),
        ("macos", "x86_64") => Some("x86_64-apple-darwin"),
        _ => None,
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn verify_file(path: &Path, expected: &str) -> Result<()> {
    let bytes = fs::read(path).map_err(|error| {
        RuntimeInstallError::new(
            RuntimeErrorCode::IntegrityFailure,
            format!("failed to read artifact {}: {error}", path.display()),
        )
    })?;
    if sha256_hex(&bytes) != expected.to_ascii_lowercase() {
        return Err(RuntimeInstallError::new(
            RuntimeErrorCode::IntegrityFailure,
            format!("artifact SHA256 mismatch: {}", path.display()),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::*;
    use std::cell::RefCell;
    use std::collections::VecDeque;

    #[derive(Default)]
    struct Injections {
        commands: Option<VecDeque<Reply>>,
        calls: Vec<Vec<String>>,
        faults: Vec<&'static str>,
        corrupt_published: bool,
    }

    enum Reply {
        Output { body: Vec<u8>, exit: i32 },
        Missing,
        Writer,
        OversizeArtifact,
        PartialDownloadFailure,
    }

    thread_local! {
        static INJECTIONS: RefCell<Injections> = RefCell::new(Injections::default());
    }

    struct ResetInjections;

    impl Drop for ResetInjections {
        fn drop(&mut self) {
            INJECTIONS.with(|state| *state.borrow_mut() = Injections::default());
        }
    }

    fn inject(commands: Option<Vec<Reply>>, faults: Vec<&'static str>) -> ResetInjections {
        INJECTIONS.with(|state| {
            *state.borrow_mut() = Injections {
                commands: commands.map(Into::into),
                faults,
                ..Injections::default()
            };
        });
        ResetInjections
    }

    pub(super) fn io_fault(phase: &str) -> std::io::Result<()> {
        INJECTIONS.with(|state| {
            let mut state = state.borrow_mut();
            if let Some(index) = state.faults.iter().position(|fault| *fault == phase) {
                state.faults.remove(index);
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("injected {phase}"),
                ))
            } else {
                Ok(())
            }
        })
    }

    pub(super) fn after_rename(destination: &Path) {
        INJECTIONS.with(|state| {
            if state.borrow().corrupt_published {
                fs::write(
                    destination.join("worker").join(WORKER_SCRIPT_NAME),
                    b"post-rename damage",
                )
                .unwrap();
            }
        });
    }

    pub(super) fn command_reply(command: &mut Command) -> Option<std::process::Output> {
        INJECTIONS.with(|state| {
            let mut state = state.borrow_mut();
            let replies = state.commands.as_mut()?;
            let reply = replies
                .pop_front()
                .expect("unexpected command: no live network in unit tests");
            state.calls.push(
                command
                    .get_args()
                    .map(|arg| arg.to_string_lossy().into_owned())
                    .collect(),
            );
            match reply {
                Reply::PartialDownloadFailure => {
                    let args: Vec<_> = command.get_args().collect();
                    let destination =
                        Path::new(args[args.iter().position(|arg| *arg == "-o").unwrap() + 1]);
                    let staging_parent = destination
                        .parent()
                        .unwrap()
                        .parent()
                        .unwrap()
                        .parent()
                        .unwrap();
                    assert_eq!(
                        fs::read_dir(staging_parent).unwrap().count(),
                        1,
                        "previous attempt must be reclaimed before retry"
                    );
                    for call in &state.calls[..state.calls.len() - 1] {
                        let previous = &call[call.iter().position(|arg| arg == "-o").unwrap() + 1];
                        assert!(
                            !Path::new(previous).exists(),
                            "previous partial download survived cleanup"
                        );
                    }
                    fs::write(destination, b"partial transfer").unwrap();
                    #[cfg(unix)]
                    use std::os::unix::process::ExitStatusExt;
                    #[cfg(windows)]
                    use std::os::windows::process::ExitStatusExt;
                    #[cfg(windows)]
                    let status = std::process::ExitStatus::from_raw(28);
                    #[cfg(unix)]
                    let status = std::process::ExitStatus::from_raw(28 << 8);
                    Some(std::process::Output {
                        status,
                        stdout: Vec::new(),
                        stderr: Vec::new(),
                    })
                }
                Reply::OversizeArtifact => {
                    let args: Vec<_> = command.get_args().collect();
                    let destination = args[args.iter().position(|arg| *arg == "-o").unwrap() + 1];
                    File::create(destination)
                        .unwrap()
                        .set_len(DEFAULT_MAX_ARTIFACT_BYTES + 1)
                        .unwrap();
                    #[cfg(unix)]
                    use std::os::unix::process::ExitStatusExt;
                    #[cfg(windows)]
                    use std::os::windows::process::ExitStatusExt;
                    Some(std::process::Output {
                        status: std::process::ExitStatus::from_raw(0),
                        stdout: Vec::new(),
                        stderr: Vec::new(),
                    })
                }
                Reply::Output { body, exit } => {
                    #[cfg(unix)]
                    use std::os::unix::process::ExitStatusExt;
                    #[cfg(windows)]
                    use std::os::windows::process::ExitStatusExt;
                    #[cfg(windows)]
                    let status = std::process::ExitStatus::from_raw(exit as u32);
                    #[cfg(unix)]
                    let status = std::process::ExitStatus::from_raw(exit << 8);
                    Some(std::process::Output {
                        status,
                        stdout: body,
                        stderr: Vec::new(),
                    })
                }
                Reply::Missing => {
                    *command = Command::new(
                        std::env::current_exe()
                            .unwrap()
                            .parent()
                            .unwrap()
                            .join("missing-runtime-fixture-tool"),
                    );
                    None
                }
                Reply::Writer => {
                    let args: Vec<_> = command.get_args().map(|arg| arg.to_owned()).collect();
                    let destination = &args[args.iter().position(|arg| arg == "-C").unwrap() + 1];
                    *command = Command::new(std::env::current_exe().unwrap());
                    command
                        .args([
                            "--exact",
                            "runtime_installer::tests::writer_fixture",
                            "--nocapture",
                        ])
                        .env("RUNTIME_INSTALLER_CHILD_FIXTURE", destination);
                    None
                }
            }
        })
    }

    fn response(body: impl AsRef<[u8]>, status: u16, exit: i32) -> Reply {
        let mut body = body.as_ref().to_vec();
        body.extend_from_slice(format!("\n{status:03}").as_bytes());
        Reply::Output { body, exit }
    }

    fn json_response(value: &JsonValue) -> Reply {
        response(value.to_string(), 200, 0)
    }

    fn assert_consumed() {
        INJECTIONS.with(|state| assert!(state.borrow().commands.as_ref().unwrap().is_empty()));
    }

    struct Fixture {
        directory: tempfile::TempDir,
        config: RuntimeConfig,
    }

    impl Fixture {
        fn new() -> Self {
            let directory = tempfile::tempdir().unwrap();
            let source = directory.path().join("source");
            fs::create_dir(&source).unwrap();
            let tree = directory.path().join("python");
            fs::create_dir_all(tree.join("bin")).unwrap();
            fs::write(tree.join("python.exe"), b"python").unwrap();
            fs::write(tree.join("bin/python3"), b"python").unwrap();
            let binary = binary_asset_name().unwrap();
            let python = format!(
                "cpython-3.13.18+20261026-{}-install_only.tar.gz",
                pbs_target().unwrap()
            );
            let wheel = "yt_dlp-2026.09.23-py3-none-any.whl";
            fs::write(source.join(binary), b"binary").unwrap();
            let mut tar = Command::new("tar");
            tar.arg("-czf")
                .arg(source.join(&python))
                .arg("-C")
                .arg(directory.path())
                .arg("python");
            run_command(
                tar,
                RuntimeErrorCode::CurlMissing,
                RuntimeErrorCode::ExtractFailure,
            )
            .unwrap();
            let mut zip = zip::ZipWriter::new(File::create(source.join(wheel)).unwrap());
            for name in ["yt_dlp/__init__.py", "yt_dlp/version.py"] {
                zip.start_file(
                    name,
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated),
                )
                .unwrap();
                zip.write_all(b"__version__ = '2026.09.23'\n").unwrap();
            }
            zip.finish().unwrap();
            let artifacts: Vec<_> = [binary, &python, wheel].iter().map(|name| {
                let path = source.join(name);
                json!({"name": name, "url": format!("file:///{}", path.to_string_lossy().replace('\\', "/").trim_start_matches('/')), "sha256": sha256_hex(&fs::read(path).unwrap())})
            }).collect();
            let path = directory.path().join("plan.json");
            fs::write(&path, json!({"schema_version": 1, "yt_dlp": "2026.09.23", "python": {"release_tag": "20261026", "python_version": "3.13.18"}, "artifacts": artifacts}).to_string()).unwrap();
            let config = RuntimeConfig {
                libs_dir: directory.path().join("libs"),
                plan_path: Some(path),
                yt_dlp_version: "2026.09.23".into(),
                python_release_tag: "20261026".into(),
                worker_yt_dlp_version: None,
            };
            Self { directory, config }
        }

        fn root(&self) -> PathBuf {
            fs::canonicalize(&self.config.libs_dir).unwrap()
        }

        fn install(&self, worker: &[u8]) -> RuntimePaths {
            install_once(&self.config, worker).unwrap()
        }

        fn no_staging(&self) {
            assert_eq!(
                fs::read_dir(self.root().join("staging")).unwrap().count(),
                0
            );
        }

        fn replace_binary(&self) {
            let path = self
                .directory
                .path()
                .join("source")
                .join(binary_asset_name().unwrap());
            fs::write(&path, b"replacement binary").unwrap();
            let plan_path = self.config.plan_path.as_ref().unwrap();
            let mut plan: JsonValue =
                serde_json::from_slice(&fs::read(plan_path).unwrap()).unwrap();
            plan["artifacts"][0]["sha256"] = json!(sha256_hex(&fs::read(path).unwrap()));
            fs::write(plan_path, plan.to_string()).unwrap();
        }
    }

    #[derive(Debug, PartialEq)]
    struct SnapshotEntry(
        PathBuf,
        bool,
        Vec<u8>,
        fs::Permissions,
        std::time::SystemTime,
    );

    fn snapshot(root: &Path) -> Vec<SnapshotEntry> {
        fn visit(root: &Path, path: &Path, result: &mut Vec<SnapshotEntry>) {
            let metadata = fs::symlink_metadata(path).unwrap();
            assert!(!is_link(&metadata));
            result.push(SnapshotEntry(
                path.strip_prefix(root).unwrap().to_owned(),
                metadata.is_dir(),
                if metadata.is_dir() {
                    Vec::new()
                } else {
                    fs::read(path).unwrap()
                },
                metadata.permissions(),
                metadata.modified().unwrap(),
            ));
            if metadata.is_dir() {
                for entry in fs::read_dir(path).unwrap() {
                    visit(root, &entry.unwrap().path(), result);
                }
            }
        }
        let mut result = Vec::new();
        visit(root, root, &mut result);
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }

    #[test]
    fn embedded_worker_identity_replaces_same_path_and_disarms_guard() {
        let fixture = Fixture::new();
        let first = fixture.install(WORKER_SCRIPT);
        let installed = fixture.root().join("runtime");
        fs::write(installed.join("old-only"), b"old").unwrap();
        let worker = b"different embedded build";
        let second = fixture.install(worker);
        assert_ne!(first.runtime_id, second.runtime_id);
        assert_eq!(first.binary_path, second.binary_path);
        assert_eq!(first.python_path, second.python_path);
        assert_eq!(first.worker_path, second.worker_path);
        assert_eq!(first.python_path_entries, second.python_path_entries);
        let plan = resolve_plan(&fixture.config).unwrap();
        assert_eq!(
            validate_runtime(&installed, &plan, &second.runtime_id, &sha256_hex(worker)).unwrap(),
            second
        );
        assert_eq!(fs::read(&second.worker_path).unwrap(), worker);
        assert!(!installed.join("old-only").exists());
        assert_eq!(fs::read_dir(fixture.root()).unwrap().count(), 2);
        fixture.no_staging();
        let before = snapshot(&fixture.root());
        fs::rename(
            fixture.directory.path().join("source"),
            fixture.directory.path().join("offline"),
        )
        .unwrap();
        assert_eq!(fixture.install(worker), second);
        assert_eq!(before, snapshot(&fixture.root()));
    }

    #[test]
    fn publication_faults_preserve_or_retire_at_the_declared_boundary() {
        let mut phases = vec!["candidate-manifest-sync", "remove-retired", "rename"];
        if cfg!(unix) {
            phases.extend(["candidate-sync", "invalidate-sync", "root-sync"]);
        }
        for phase in phases {
            let fixture = Fixture::new();
            fixture.install(WORKER_SCRIPT);
            let installed = fixture.root().join("runtime");
            fs::write(installed.join("old-only"), b"old").unwrap();
            let before = snapshot(&installed);
            fixture.replace_binary();
            let _reset = inject(None, vec![phase]);
            let error = install_with_retry(fixture.config.clone()).unwrap_err();
            assert_eq!(
                error.code,
                RuntimeErrorCode::PublishFailure,
                "{phase}: {error}"
            );
            assert!(error.message.contains(&format!("phase={phase}")));
            assert!(error.message.contains("runtime_state="));
            assert!(error.message.contains("attempt=1"));
            match phase {
                "candidate-manifest-sync" | "candidate-sync" => {
                    assert_eq!(before, snapshot(&installed))
                }
                "remove-retired" | "invalidate-sync" => {
                    assert!(!installed.join(MANIFEST_NAME).exists());
                    assert!(installed.join("old-only").exists());
                }
                "rename" => assert!(!installed.exists()),
                "root-sync" => {
                    let plan = resolve_plan(&fixture.config).unwrap();
                    let hash = sha256_hex(WORKER_SCRIPT);
                    validate_runtime(
                        &installed,
                        &plan,
                        &derive_runtime_id(&plan, &hash).unwrap(),
                        &hash,
                    )
                    .unwrap();
                    assert!(!installed.join("old-only").exists());
                }
                _ => unreachable!(),
            }
            fixture.no_staging();
            fixture.install(WORKER_SCRIPT);
            fixture.no_staging();
        }
    }

    #[test]
    fn post_publication_validation_preserves_candidate_and_never_returns_paths() {
        let fixture = Fixture::new();
        fixture.install(WORKER_SCRIPT);
        fixture.replace_binary();
        let _reset = inject(None, vec![]);
        INJECTIONS.with(|state| state.borrow_mut().corrupt_published = true);
        let error = install_with_retry(fixture.config.clone()).unwrap_err();
        assert_eq!(error.code, RuntimeErrorCode::IntegrityFailure);
        assert!(error
            .message
            .contains("phase=post-publication runtime_state=candidate"));
        assert!(error.message.contains("attempt=1"));
        assert!(fixture.root().join("runtime/manifest.json").is_file());
        fixture.no_staging();
        INJECTIONS.with(|state| state.borrow_mut().corrupt_published = false);
        fixture.install(WORKER_SCRIPT);
    }

    #[test]
    fn successful_rename_disarms_cleanup_even_when_called_explicitly() {
        let fixture = Fixture::new();
        let root = install_root(&fixture.config.libs_dir).unwrap();
        let plan = resolve_plan(&fixture.config).unwrap();
        let hash = sha256_hex(WORKER_SCRIPT);
        let id = derive_runtime_id(&plan, &hash).unwrap();
        let mut staging = OwnedStaging::create(&root).unwrap();
        prepare_staging(&staging.path, &plan, WORKER_SCRIPT).unwrap();
        write_manifest(&staging.path, &plan, &id, &hash).unwrap();
        sync_directory(&staging.path, "candidate-sync", "absent").unwrap();
        let installed = root.join("runtime");
        let paths = publish(&mut staging, &installed, None, &plan, &id, &hash).unwrap();
        assert!(staging.published);
        let before = snapshot(&installed);
        staging.cleanup().unwrap();
        drop(staging);
        assert_eq!(before, snapshot(&installed));
        assert_eq!(
            validate_runtime(&installed, &plan, &id, &hash).unwrap(),
            paths
        );
        fixture.no_staging();
    }

    #[test]
    fn observed_runtime_interference_is_not_overwritten() {
        let fixture = Fixture::new();
        let root = install_root(&fixture.config.libs_dir).unwrap();
        let mut staging = OwnedStaging::create(&root).unwrap();
        let plan = resolve_plan(&fixture.config).unwrap();
        let hash = sha256_hex(WORKER_SCRIPT);
        let id = derive_runtime_id(&plan, &hash).unwrap();
        prepare_staging(&staging.path, &plan, WORKER_SCRIPT).unwrap();
        write_manifest(&staging.path, &plan, &id, &hash).unwrap();
        let destination = root.join("runtime");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("foreign"), b"not ours").unwrap();
        let before = snapshot(&destination);
        let error = publish(&mut staging, &destination, None, &plan, &id, &hash).unwrap_err();
        assert_eq!(error.code, RuntimeErrorCode::PublishFailure);
        assert!(error
            .message
            .contains("phase=pre-retirement runtime_state=unknown"));
        assert_eq!(before, snapshot(&destination));
        staging.cleanup().unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn native_unix_denial_stops_invalidation_and_preserves_owned_cleanup_failure() {
        use std::os::unix::fs::PermissionsExt;
        let fixture = Fixture::new();
        fixture.install(WORKER_SCRIPT);
        let installed = fixture.root().join("runtime");
        fixture.replace_binary();
        fs::set_permissions(&installed, fs::Permissions::from_mode(0o555)).unwrap();
        // Privileged test runners can bypass chmod; they cannot supply denial evidence.
        if File::create(installed.join("privilege-probe")).is_ok() {
            fs::set_permissions(&installed, fs::Permissions::from_mode(0o755)).unwrap();
            eprintln!(
                "native Unix denial assertions unavailable: runner bypasses directory permissions"
            );
            return;
        }
        let before = snapshot(&installed);
        let result = install_with_retry(fixture.config.clone());
        let after = snapshot(&installed);
        fs::set_permissions(&installed, fs::Permissions::from_mode(0o755)).unwrap();
        let error = result.unwrap_err();
        assert_eq!(error.code, RuntimeErrorCode::PublishFailure);
        assert!(error.message.contains("phase=invalidate-manifest"));
        assert_eq!(before, after);
        fixture.no_staging();
        let staging = OwnedStaging::create(&fixture.root()).unwrap();
        fs::write(staging.path.join("partial"), b"owned partial").unwrap();
        fs::set_permissions(&staging.path, fs::Permissions::from_mode(0o555)).unwrap();
        let result = staging.cleanup();
        fs::set_permissions(&staging.path, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(result.unwrap_err().code, RuntimeErrorCode::StagingFailure);
        staging.cleanup().unwrap();
    }

    #[test]
    fn cleanup_failure_retains_primary_and_prevents_download_retry() {
        let fixture = Fixture::new();
        fixture.install(WORKER_SCRIPT);
        fixture.replace_binary();
        fs::remove_file(
            fixture
                .directory
                .path()
                .join("source")
                .join(binary_asset_name().unwrap()),
        )
        .unwrap();
        let before = snapshot(&fixture.root().join("runtime"));
        let _reset = inject(None, vec!["cleanup"]);
        let error = install_with_retry(fixture.config.clone()).unwrap_err();
        assert_eq!(error.code, RuntimeErrorCode::StagingFailure);
        assert!(error.message.contains("attempt=1"));
        assert!(error.message.contains("primary=[RUNTIME_DOWNLOAD_FAILURE"));
        assert!(error.message.contains("phase=cleanup path="));
        assert_eq!(before, snapshot(&fixture.root().join("runtime")));
        let paths: Vec<_> = fs::read_dir(fixture.root().join("staging"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(paths.len(), 1);
        assert!(!paths[0].join(MANIFEST_NAME).exists());
    }

    #[test]
    fn uncertain_child_quiescence_preserves_owned_work_without_retry() {
        let fixture = Fixture::new();
        let _reset = inject(None, vec!["child-wait"]);
        let error = install_with_retry(fixture.config.clone()).unwrap_err();
        assert_eq!(error.code, RuntimeErrorCode::StagingFailure);
        assert!(error.message.contains("attempt=1"));
        assert!(error.message.contains("primary=[RUNTIME_STAGING_FAILURE"));
        assert!(error.message.contains("quiescence is unknown; preserved"));
        assert!(!fixture.root().join("runtime").exists());
        assert_eq!(
            fs::read_dir(fixture.root().join("staging"))
                .unwrap()
                .count(),
            1
        );
    }

    #[test]
    fn writer_fixture() {
        let Some(path) = std::env::var_os("RUNTIME_INSTALLER_CHILD_FIXTURE") else {
            return;
        };
        let path = PathBuf::from(path);
        fs::write(path.join("writer-started"), b"started").unwrap();
        sleep(Duration::from_millis(50));
        fs::write(path.join("writer-finished"), b"finished").unwrap();
        std::process::exit(7);
    }

    #[test]
    fn failed_local_writer_is_awaited_before_manifest_free_cleanup() {
        let fixture = Fixture::new();
        let root = install_root(&fixture.config.libs_dir).unwrap();
        let staging = OwnedStaging::create(&root).unwrap();
        let plan = resolve_plan(&fixture.config).unwrap();
        let _reset = inject(Some(vec![Reply::Writer]), vec![]);
        let error = prepare_staging(&staging.path, &plan, WORKER_SCRIPT).unwrap_err();
        assert_eq!(error.code, RuntimeErrorCode::ExtractFailure);
        assert!(error.message.contains("exit status Some(7)"));
        assert!(error.cleanup_safe);
        assert_eq!(
            fs::read(staging.path.join("writer-finished")).unwrap(),
            b"finished"
        );
        assert!(!staging.path.join(MANIFEST_NAME).exists());
        staging.cleanup().unwrap();
        assert!(!staging.path.exists());
        assert_consumed();
    }

    #[cfg(windows)]
    #[test]
    fn native_locked_staging_reports_cleanup_error_then_allows_owned_cleanup() {
        use std::os::windows::fs::OpenOptionsExt;
        let fixture = tempfile::tempdir().unwrap();
        let root = install_root(fixture.path()).unwrap();
        let staging = OwnedStaging::create(&root).unwrap();
        let path = staging.path.join("partial");
        fs::write(&path, b"partial").unwrap();
        let handle = File::options()
            .read(true)
            .share_mode(1)
            .open(&path)
            .unwrap();
        let error = staging.cleanup().unwrap_err();
        assert_eq!(error.code, RuntimeErrorCode::StagingFailure);
        assert!(path.exists());
        drop(handle);
        staging.cleanup().unwrap();
        assert!(!staging.path.exists());
    }

    fn release_metadata(name: &str, digest: Option<JsonValue>) -> JsonValue {
        let mut asset = json!({"name": name, "browser_download_url": format!("https://fixtures.invalid/{name}")});
        if let Some(digest) = digest {
            asset["digest"] = digest;
        }
        json!({"assets": [asset, {"name": "SHA2-256SUMS", "browser_download_url": "https://fixtures.invalid/SHA2-256SUMS"}]})
    }

    fn wheel_metadata() -> JsonValue {
        json!({"urls": [{"filename": "yt_dlp-2026.09.23-py3-none-any.whl", "packagetype": "bdist_wheel", "url": "https://fixtures.invalid/wheel", "digests": {"sha256": "a".repeat(64)}}]})
    }

    fn unusable_digests() -> Vec<Option<JsonValue>> {
        vec![
            None,
            Some(JsonValue::Null),
            Some(json!(7)),
            Some(json!("")),
            Some(json!("a".repeat(63))),
            Some(json!("z".repeat(64))),
        ]
    }

    #[test]
    fn release_digest_preference_fallback_and_exhaustion_use_real_resolver() {
        let name = binary_asset_name().unwrap();
        let valid = format!("sha256:{}", "A".repeat(64));
        let _reset = inject(
            Some(vec![json_response(&release_metadata(
                name,
                Some(json!(valid)),
            ))]),
            vec![],
        );
        let artifact =
            github_release_artifact("yt-dlp/yt-dlp", "2026.09.23", name, ArtifactKind::Binary)
                .unwrap();
        assert_eq!(artifact.sha256, "a".repeat(64));
        assert_consumed();
        for digest in unusable_digests()
            .into_iter()
            .chain([Some(json!("sha256:bad")), Some(json!("md5:wrong"))])
        {
            let metadata = release_metadata(name, digest.clone());
            let _reset = inject(
                Some(vec![
                    json_response(&metadata),
                    response(format!("{} {name}\n", "B".repeat(64)), 200, 0),
                ]),
                vec![],
            );
            assert_eq!(
                github_release_artifact("yt-dlp/yt-dlp", "2026.09.23", name, ArtifactKind::Binary)
                    .unwrap()
                    .sha256,
                "b".repeat(64)
            );
            assert_consumed();
            let _reset = inject(Some(vec![json_response(&metadata)]), vec![]);
            let error = github_release_artifact(
                "astral-sh/python-build-standalone",
                "20261026",
                name,
                ArtifactKind::Python,
            )
            .unwrap_err();
            assert_eq!(error.code, RuntimeErrorCode::ResolveFailure);
            assert_consumed();
        }
        for body in [
            vec![0xff],
            b"".to_vec(),
            b"not-a-hash yt-dlp.exe".to_vec(),
            format!("{} wrong-name", "b".repeat(64)).into_bytes(),
        ] {
            let _reset = inject(
                Some(vec![
                    json_response(&release_metadata(name, None)),
                    response(body, 200, 0),
                ]),
                vec![],
            );
            assert_eq!(
                github_release_artifact("yt-dlp/yt-dlp", "2026.09.23", name, ArtifactKind::Binary)
                    .unwrap_err()
                    .code,
                RuntimeErrorCode::ResolveFailure
            );
            assert_consumed();
        }
        for (status, exit, code) in [
            (404, 22, RuntimeErrorCode::ResolveFailure),
            (403, 22, RuntimeErrorCode::DownloadFailure),
            (500, 22, RuntimeErrorCode::DownloadFailure),
            (0, 28, RuntimeErrorCode::DownloadFailure),
            (0, 6, RuntimeErrorCode::DownloadFailure),
        ] {
            let _reset = inject(
                Some(vec![
                    json_response(&release_metadata(name, None)),
                    response([], status, exit),
                ]),
                vec![],
            );
            let error =
                github_release_artifact("yt-dlp/yt-dlp", "2026.09.23", name, ArtifactKind::Binary)
                    .unwrap_err();
            assert_eq!(error.code, code);
            assert_eq!(error.http_status, Some(status));
            assert!(error.message.contains(&format!("exit status Some({exit})")));
            assert_consumed();
        }
    }

    #[test]
    fn release_and_python_metadata_fail_closed() {
        let name = binary_asset_name().unwrap();
        for index in [0, 1] {
            let mut metadata = release_metadata(name, None);
            let duplicate = metadata["assets"][index].clone();
            metadata["assets"].as_array_mut().unwrap().push(duplicate);
            let _reset = inject(Some(vec![json_response(&metadata)]), vec![]);
            assert_eq!(
                github_release_artifact("yt-dlp/yt-dlp", "2026.09.23", name, ArtifactKind::Binary)
                    .unwrap_err()
                    .code,
                RuntimeErrorCode::ResolveFailure
            );
            assert_consumed();
        }
        for metadata in [
            json!({}),
            json!({"assets": null}),
            json!({"assets": []}),
            json!({"assets": [{"name": "wrong"}]}),
            json!({"assets": [{"name": name}]}),
            release_metadata(name, None),
        ] {
            let mut metadata = metadata;
            if let Some(assets) = metadata.get_mut("assets").and_then(JsonValue::as_array_mut) {
                assets.retain(|asset| asset["name"] != "SHA2-256SUMS");
            }
            let _reset = inject(Some(vec![json_response(&metadata)]), vec![]);
            assert_eq!(
                github_release_artifact("yt-dlp/yt-dlp", "2026.09.23", name, ArtifactKind::Binary)
                    .unwrap_err()
                    .code,
                RuntimeErrorCode::ResolveFailure
            );
            assert_consumed();
        }
        for invalid_url in [
            JsonValue::Null,
            json!(7),
            json!(""),
            json!("http://wrong"),
            json!("https://"),
        ] {
            for sums in [false, true] {
                let mut metadata = release_metadata(
                    name,
                    if sums {
                        None
                    } else {
                        Some(json!(format!("sha256:{}", "a".repeat(64))))
                    },
                );
                metadata["assets"][usize::from(sums)]["browser_download_url"] = invalid_url.clone();
                let _reset = inject(Some(vec![json_response(&metadata)]), vec![]);
                assert_eq!(
                    github_release_artifact(
                        "yt-dlp/yt-dlp",
                        "2026.09.23",
                        name,
                        ArtifactKind::Binary
                    )
                    .unwrap_err()
                    .code,
                    RuntimeErrorCode::ResolveFailure
                );
                assert_consumed();
            }
        }
        let target = pbs_target().unwrap();
        let duplicate_pbs = json!({"assets": [
            {"name": format!("cpython-3.13.18+20261026-{target}-install_only.tar.gz")},
            {"name": format!("cpython-3.13.19+20261026-{target}-install_only.tar.gz")}
        ]});
        let _reset = inject(Some(vec![json_response(&duplicate_pbs)]), vec![]);
        assert_eq!(
            resolve_live_python_version("20261026", target)
                .unwrap_err()
                .code,
            RuntimeErrorCode::ResolveFailure
        );
        assert_consumed();
        for asset in [
            json!({}),
            json!({"name": 1}),
            json!({"name": format!("cpython-bad+20261026-{target}-install_only.tar.gz")}),
            json!({"name": format!("cpython-3.13.18+wrong-{target}-install_only.tar.gz")}),
        ] {
            let _reset = inject(
                Some(vec![json_response(&json!({"assets": [asset]}))]),
                vec![],
            );
            assert_eq!(
                resolve_live_python_version("20261026", target)
                    .unwrap_err()
                    .code,
                RuntimeErrorCode::ResolveFailure
            );
            assert_consumed();
        }
        let _reset = inject(Some(vec![response(b"not JSON", 200, 0)]), vec![]);
        assert_eq!(
            resolve_live_python_version("20261026", target)
                .unwrap_err()
                .code,
            RuntimeErrorCode::ResolveFailure
        );
    }

    #[test]
    fn wheel_metadata_requires_one_exact_wheel_and_usable_digest() {
        let expected = "yt_dlp-2026.09.23-py3-none-any.whl";
        let base = wheel_metadata();
        let mut cases = vec![
            json!({}),
            json!({"urls": null}),
            json!({"urls": []}),
            json!({"urls": [base["urls"][0].clone(), base["urls"][0].clone()]}),
        ];
        for digest in unusable_digests() {
            let mut metadata = base.clone();
            if let Some(digest) = digest {
                metadata["urls"][0]["digests"]["sha256"] = digest;
            } else {
                metadata["urls"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("digests");
            }
            cases.push(metadata);
        }
        for field in ["filename", "packagetype", "url"] {
            let mut missing = base.clone();
            missing["urls"][0].as_object_mut().unwrap().remove(field);
            cases.push(missing);
            for value in [JsonValue::Null, json!(8), json!(""), json!("wrong")] {
                let mut metadata = base.clone();
                metadata["urls"][0][field] = value;
                cases.push(metadata);
            }
        }
        let mut wrong_name = base.clone();
        wrong_name["urls"][0]["filename"] = json!("yt_dlp-other-py3-none-any.whl");
        cases.push(wrong_name);
        for metadata in cases {
            let _reset = inject(Some(vec![json_response(&metadata)]), vec![]);
            assert_eq!(
                pypi_wheel_artifact("2026.09.23", expected)
                    .unwrap_err()
                    .code,
                RuntimeErrorCode::ResolveFailure,
                "{metadata}"
            );
            assert_consumed();
        }
        let _reset = inject(Some(vec![json_response(&base)]), vec![]);
        let wheel = pypi_wheel_artifact("2026.09.23", expected).unwrap();
        assert_eq!(wheel.name, expected);
        assert_eq!(wheel.sha256, "a".repeat(64));
        assert_consumed();
    }

    #[test]
    fn live_resolution_uses_pinned_channels_and_transfer_arguments() {
        let mut config = RuntimeConfig {
            libs_dir: PathBuf::new(),
            plan_path: None,
            yt_dlp_version: "2026.09.23".into(),
            python_release_tag: "20261026".into(),
            worker_yt_dlp_version: None,
        };
        let python = format!(
            "cpython-3.13.18+20261026-{}-install_only.tar.gz",
            pbs_target().unwrap()
        );
        let pbs = release_metadata(&python, Some(json!(format!("sha256:{}", "b".repeat(64)))));
        let _reset = inject(
            Some(vec![
                json_response(&pbs),
                json_response(&release_metadata(
                    binary_asset_name().unwrap(),
                    Some(json!(format!("sha256:{}", "a".repeat(64)))),
                )),
                json_response(&pbs),
                json_response(&wheel_metadata()),
            ]),
            vec![],
        );
        let plan = resolve_plan(&config).unwrap();
        assert_eq!(plan.python_version, "3.13.18");
        assert_eq!(plan.artifacts.len(), 3);
        assert_eq!(plan.artifacts[1].name, python);
        assert_consumed();
        INJECTIONS.with(|state| {
            let state = state.borrow();
            for args in &state.calls {
                assert_eq!(&args[..7], ["-fsSL", "--retry", "0", "--max-time", "300", "--max-filesize", "67108864"]);
                assert!(!args.iter().any(|arg| arg.contains("proxy") || arg.contains("cacert") || arg.contains("latest")));
            }
            assert_eq!(state.calls[0].last().unwrap(), "https://api.github.com/repos/astral-sh/python-build-standalone/releases/tags/20261026");
            assert_eq!(state.calls[1].last().unwrap(), "https://api.github.com/repos/yt-dlp/yt-dlp/releases/tags/2026.09.23");
            assert_eq!(state.calls[3].last().unwrap(), "https://pypi.org/pypi/yt-dlp/2026.09.23/json");
        });
        drop(_reset);
        let fixture = Fixture::new();
        config.plan_path = fixture.config.plan_path.clone();
        let _reset = inject(Some(vec![]), vec![]);
        resolve_plan(&config).unwrap();
        assert_consumed();
    }

    #[test]
    fn plan_digest_and_configuration_errors_do_not_retry_or_create_staging() {
        let fixture = Fixture::new();
        let path = fixture.config.plan_path.as_ref().unwrap();
        let base: JsonValue = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        let _reset = inject(Some(vec![]), vec![]);
        for index in 0..3 {
            for digest in unusable_digests() {
                let mut plan = base.clone();
                if let Some(digest) = digest {
                    plan["artifacts"][index]["sha256"] = digest;
                } else {
                    plan["artifacts"][index]
                        .as_object_mut()
                        .unwrap()
                        .remove("sha256");
                }
                fs::write(path, plan.to_string()).unwrap();
                let error = install_with_retry(fixture.config.clone()).unwrap_err();
                assert_eq!(error.code, RuntimeErrorCode::ConfigInvalid);
                assert!(error.message.contains("attempt=1"));
                assert!(!fixture.root().join("staging").exists());
            }
        }
        for body in ["invalid JSON", "{}", "null"] {
            fs::write(path, body).unwrap();
            assert_eq!(
                install_with_retry(fixture.config.clone()).unwrap_err().code,
                RuntimeErrorCode::ConfigInvalid
            );
        }
        fs::write(path, base.to_string()).unwrap();
        for field in [
            "/schema_version",
            "/yt_dlp",
            "/python",
            "/python/python_version",
            "/python/release_tag",
            "/artifacts",
            "/artifacts/0/name",
            "/artifacts/1/name",
            "/artifacts/2/name",
            "/artifacts/0/url",
        ] {
            for value in [JsonValue::Null, json!(""), json!(42), json!("wrong")] {
                let mut plan = base.clone();
                *plan.pointer_mut(field).unwrap() = value;
                fs::write(path, plan.to_string()).unwrap();
                assert_eq!(
                    install_with_retry(fixture.config.clone()).unwrap_err().code,
                    RuntimeErrorCode::ConfigInvalid,
                    "{field}"
                );
                assert!(!fixture.root().join("staging").exists());
            }
        }
        for index in 0..3 {
            let mut plan = base.clone();
            let name = plan["artifacts"][index]["name"].as_str().unwrap();
            plan["artifacts"][index]["name"] = json!(if index == 0 {
                "yt-dlp_macos".to_string()
            } else {
                name.replace("2026", "2025")
            });
            // The fixture host may itself be macOS; choose another platform then.
            if index == 0 && binary_asset_name() == Some("yt-dlp_macos") {
                plan["artifacts"][0]["name"] = json!("yt-dlp.exe");
            }
            fs::write(path, plan.to_string()).unwrap();
            assert_eq!(
                resolve_plan(&fixture.config).unwrap_err().code,
                RuntimeErrorCode::ConfigInvalid
            );
        }
        fs::write(path, base.to_string()).unwrap();
        for (version, release, worker, expected) in [
            ("", "20261026", None, RuntimeErrorCode::ConfigInvalid),
            (
                "2026.09.23",
                "latest",
                None,
                RuntimeErrorCode::ConfigInvalid,
            ),
            (
                "2026.09.23",
                "20261026",
                Some("different"),
                RuntimeErrorCode::VersionMismatch,
            ),
        ] {
            let mut config = fixture.config.clone();
            config.yt_dlp_version = version.into();
            config.python_release_tag = release.into();
            config.worker_yt_dlp_version = worker.map(str::to_owned);
            assert_eq!(install_with_retry(config).unwrap_err().code, expected);
        }
        assert_consumed();
    }

    #[test]
    fn partial_download_is_cleaned_before_each_bounded_retry() {
        let fixture = Fixture::new();
        let path = fixture.config.plan_path.as_ref().unwrap();
        let mut plan: JsonValue = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        plan["artifacts"][0]["url"] = json!("https://fixtures.invalid/binary");
        fs::write(path, plan.to_string()).unwrap();
        let _reset = inject(
            Some((0..3).map(|_| Reply::PartialDownloadFailure).collect()),
            vec![],
        );
        let error = install_with_retry(fixture.config.clone()).unwrap_err();
        assert_eq!(error.code, RuntimeErrorCode::DownloadFailure);
        assert!(error.message.contains("attempt=3"));
        assert!(error.message.contains("artifact="));
        fixture.no_staging();
        assert!(!fixture.root().join("runtime").exists());
        assert_consumed();
    }

    #[test]
    fn unsafe_wheel_entry_is_an_extraction_error() {
        let fixture = tempfile::tempdir().unwrap();
        let wheel = fixture.path().join("unsafe.whl");
        let mut zip = zip::ZipWriter::new(File::create(&wheel).unwrap());
        zip.start_file("../outside", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"not permitted").unwrap();
        zip.finish().unwrap();
        let destination = fixture.path().join("site-packages");
        fs::create_dir(&destination).unwrap();
        let error = extract_zip(&wheel, &destination).unwrap_err();
        assert_eq!(error.code, RuntimeErrorCode::ExtractFailure);
        assert!(!fixture.path().join("outside").exists());
    }

    #[test]
    fn initial_manifest_sync_failure_never_exposes_partial_runtime() {
        let fixture = Fixture::new();
        let _reset = inject(None, vec!["candidate-manifest-sync"]);
        let error = install_with_retry(fixture.config.clone()).unwrap_err();
        assert_eq!(error.code, RuntimeErrorCode::PublishFailure);
        assert!(error.message.contains("phase=candidate-manifest-sync"));
        assert!(!fixture.root().join("runtime").exists());
        fixture.no_staging();
        fixture.install(WORKER_SCRIPT);
    }

    #[cfg(unix)]
    #[test]
    fn unix_executable_permission_failure_is_not_retryable() {
        let fixture = tempfile::tempdir().unwrap();
        assert_eq!(
            set_executable(&fixture.path().join("missing"))
                .unwrap_err()
                .code,
            RuntimeErrorCode::PermissionFailure
        );
    }

    #[test]
    fn missing_tools_and_nonzero_tar_use_distinct_error_codes() {
        let fixture = Fixture::new();
        let _reset = inject(Some(vec![Reply::Missing]), vec![]);
        let error = curl_to_memory("https://fixtures.invalid/metadata", 16).unwrap_err();
        assert_eq!(error.code, RuntimeErrorCode::CurlMissing);
        assert!(error.message.contains("tool=curl"));
        assert_consumed();
        let _reset = inject(Some(vec![Reply::Missing]), vec![]);
        let error = install_with_retry(fixture.config.clone()).unwrap_err();
        assert_eq!(error.code, RuntimeErrorCode::CurlMissing);
        assert!(error.message.contains("tool=tar"));
        assert!(error.message.contains("attempt=1"));
        fixture.no_staging();
        assert_consumed();
        let _reset = inject(
            Some(vec![Reply::Output {
                body: Vec::new(),
                exit: 2,
            }]),
            vec![],
        );
        let error = install_with_retry(fixture.config.clone()).unwrap_err();
        assert_eq!(error.code, RuntimeErrorCode::ExtractFailure);
        assert!(error
            .message
            .contains("tool=tar failed with exit status Some(2)"));
        assert!(error.message.contains("attempt=1"));
        fixture.no_staging();
        assert_consumed();
    }

    #[test]
    fn download_status_limits_and_sensitive_urls_use_real_transfer_logic() {
        for exit in [22, 28, 63, 6] {
            let _reset = inject(Some(vec![response([], 0, exit)]), vec![]);
            let error = curl_to_memory("https://user:secret@fixtures.invalid/?token=hidden", 16)
                .unwrap_err();
            assert_eq!(error.code, RuntimeErrorCode::DownloadFailure);
            assert!(error.message.contains(&format!("Some({exit})")));
            for secret in ["secret", "hidden", "user:"] {
                assert!(!error.message.contains(secret));
            }
        }
        let _reset = inject(Some(vec![response(b"oversize", 200, 0)]), vec![]);
        assert_eq!(
            curl_to_memory("https://fixtures.invalid/metadata", 2)
                .unwrap_err()
                .code,
            RuntimeErrorCode::DownloadFailure
        );
        drop(_reset);
        let fixture = Fixture::new();
        let mut artifact = resolve_plan(&fixture.config).unwrap().artifacts.remove(0);
        artifact.url = "https://fixtures.invalid/binary".into();
        let _reset = inject(Some(vec![Reply::OversizeArtifact]), vec![]);
        let error =
            fetch_to_file(&artifact, &fixture.directory.path().join("oversize")).unwrap_err();
        assert_eq!(error.code, RuntimeErrorCode::DownloadFailure);
        assert!(error.message.contains("exceeds the size limit"));
        assert_consumed();
    }

    #[test]
    fn only_download_failures_retry_and_metadata_404_exhaustion_stops() {
        let fixture = Fixture::new();
        let mut config = fixture.config.clone();
        config.plan_path = None;
        let _reset = inject(Some((0..3).map(|_| response([], 0, 28)).collect()), vec![]);
        let error = install_with_retry(config.clone()).unwrap_err();
        assert_eq!(error.code, RuntimeErrorCode::DownloadFailure);
        assert!(error.message.contains("attempt=3"));
        assert_consumed();
        assert!(!fixture.root().join("runtime").exists());
        assert!(!fixture.root().join("staging").exists());
        let python = format!(
            "cpython-3.13.18+20261026-{}-install_only.tar.gz",
            pbs_target().unwrap()
        );
        let _reset = inject(
            Some(vec![
                json_response(&release_metadata(&python, None)),
                json_response(&release_metadata(binary_asset_name().unwrap(), None)),
                response([], 404, 22),
            ]),
            vec![],
        );
        let error = install_with_retry(config).unwrap_err();
        assert_eq!(error.code, RuntimeErrorCode::ResolveFailure);
        assert!(error.message.contains("attempt=1"));
        assert_consumed();
    }

    #[test]
    fn owned_partial_staging_cleanup_needs_no_manifest() {
        let fixture = tempfile::tempdir().unwrap();
        let root = install_root(fixture.path()).unwrap();
        let staging = OwnedStaging::create(&root).unwrap();
        fs::write(staging.path.join("partial"), b"partial download").unwrap();
        assert!(!staging.path.join(MANIFEST_NAME).exists());
        staging.cleanup().unwrap();
        assert!(!staging.path.exists());
    }

    #[test]
    fn uncertain_work_is_preserved_on_guard_drop() {
        let fixture = tempfile::tempdir().unwrap();
        let root = install_root(fixture.path()).unwrap();
        let staging = OwnedStaging::create(&root).unwrap();
        let path = staging.path.clone();
        fs::write(path.join("partial"), b"uncertain writer").unwrap();
        drop(staging);
        assert_eq!(fs::read(path.join("partial")).unwrap(), b"uncertain writer");
    }

    #[test]
    fn changed_staging_parent_is_not_cleanup_authority() {
        let fixture = tempfile::tempdir().unwrap();
        let root = install_root(fixture.path()).unwrap();
        let staging = OwnedStaging::create(&root).unwrap();
        let original_parent = root.join("staging");
        let moved_parent = root.join("interference");
        // Both rename targets are direct children of this isolated fixture.
        assert_eq!(original_parent.parent(), Some(root.as_path()));
        assert_eq!(moved_parent.parent(), Some(root.as_path()));
        fs::rename(&original_parent, &moved_parent).unwrap();
        fs::create_dir(&original_parent).unwrap();
        fs::write(original_parent.join("foreign"), b"leave alone").unwrap();
        assert!(staging.cleanup().is_err());
        assert!(original_parent.join("foreign").exists());
        assert!(moved_parent
            .join(staging.path.file_name().unwrap())
            .exists());
    }

    #[cfg(unix)]
    #[test]
    fn staging_cleanup_unlinks_without_following_links() {
        let fixture = tempfile::tempdir().unwrap();
        let root = install_root(fixture.path()).unwrap();
        let staging = OwnedStaging::create(&root).unwrap();
        let outside = root.join("foreign");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("keep"), b"foreign bytes").unwrap();
        std::os::unix::fs::symlink(&outside, staging.path.join("link")).unwrap();
        staging.cleanup().unwrap();
        assert_eq!(fs::read(outside.join("keep")).unwrap(), b"foreign bytes");
    }
}
