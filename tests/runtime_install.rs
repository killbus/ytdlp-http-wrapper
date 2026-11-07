#![allow(clippy::expect_used, clippy::unwrap_used)]

//! Deterministic, offline acceptance tests for the project-owned immutable
//! runtime installer. The plan file points at file:// fixtures, so no test
//! touches the network.

use std::fs;
#[cfg(windows)]
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use ytdlp_http_wrapper::runtime_installer::{ensure_runtime, RuntimeConfig, RuntimePaths};

const VERSION: &str = "2026.09.23";
#[path = "fixtures/managed_runtime.rs"]
mod managed;

struct Fixture {
    binary: PathBuf,
    wheel: PathBuf,
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn file_sha256(path: &Path) -> String {
    managed::hash(path)
}

fn binary_name() -> &'static str {
    if cfg!(windows) {
        "yt-dlp.exe"
    } else if cfg!(target_os = "macos") {
        "yt-dlp_macos"
    } else if cfg!(target_arch = "aarch64") {
        "yt-dlp_linux_aarch64"
    } else if cfg!(target_arch = "arm") {
        "yt-dlp_linux_armv7l"
    } else {
        "yt-dlp_linux"
    }
}

fn wheel_name() -> String {
    format!("yt_dlp-{}-py3-none-any.whl", VERSION)
}

use managed::file_url;

fn build_fixtures(root: &Path) -> Fixture {
    let fixtures = root.join("fixtures");
    fs::create_dir_all(&fixtures).expect("create fixtures");

    let binary = fixtures.join(binary_name());
    fs::write(&binary, b"fake yt-dlp binary\n").expect("write binary fixture");

    let wheel = fixtures.join(wheel_name());
    write_wheel(&wheel);

    Fixture { binary, wheel }
}

fn write_wheel(path: &Path) {
    write_wheel_version(path, VERSION);
}
fn write_wheel_version(path: &Path, version: &str) {
    managed::wheel(path, version, ">=3.10", "");
}

fn plan_json(fixture: &Fixture, binary_sha: Option<&str>, missing_python: bool) -> String {
    let wheel_url = if missing_python {
        file_url(&fixture.wheel.with_extension("missing.whl"))
    } else {
        file_url(&fixture.wheel)
    };
    json!({
        "schema_version": 2,
        "offline": true,
        "yt_dlp": VERSION,
        "artifacts": [
            {
                "name": binary_name(),
                "url": file_url(&fixture.binary),
                "sha256": binary_sha.unwrap_or("").to_owned()
            },
            {
                "name": wheel_name(),
                "url": wheel_url,
                "sha256": file_sha256(&fixture.wheel)
            }
        ]
    })
    .to_string()
}

fn write_plan(root: &Path, contents: &str) -> PathBuf {
    let path = root.join("runtime-plan.json");
    fs::write(&path, contents).expect("write plan");
    path
}

fn config(root: &Path, plan: &Path) -> RuntimeConfig {
    RuntimeConfig {
        libs_dir: root.join("libs"),
        plan_path: Some(plan.to_owned()),
        yt_dlp_version: VERSION.to_owned(),
        python_release_tag: None,
        python_request: Some(managed::host_python()),
        uv_path: "uv".into(),
        worker_yt_dlp_version: Some(VERSION.to_owned()),
    }
}

#[derive(Debug, PartialEq)]
struct SnapshotEntry {
    path: PathBuf,
    directory: bool,
    bytes: Vec<u8>,
    permissions: fs::Permissions,
    modified: std::time::SystemTime,
}

fn snapshot(root: &Path) -> Vec<SnapshotEntry> {
    fn visit(root: &Path, path: &Path, entries: &mut Vec<SnapshotEntry>) {
        let metadata = fs::symlink_metadata(path).expect("snapshot metadata");
        entries.push(SnapshotEntry {
            path: path.strip_prefix(root).unwrap().to_owned(),
            directory: metadata.is_dir(),
            bytes: if metadata.file_type().is_symlink() {
                fs::read_link(path)
                    .unwrap()
                    .to_string_lossy()
                    .as_bytes()
                    .to_vec()
            } else if metadata.is_dir() {
                Vec::new()
            } else {
                fs::read(path).unwrap()
            },
            permissions: metadata.permissions(),
            modified: metadata.modified().unwrap(),
        });
        if metadata.is_dir() {
            for entry in fs::read_dir(path).unwrap() {
                visit(root, &entry.unwrap().path(), entries);
            }
        }
    }
    let mut entries = Vec::new();
    visit(root, root, &mut entries);
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    entries
}

fn runtime_dir(root: &Path) -> PathBuf {
    fs::canonicalize(root.join("libs")).unwrap().join("runtime")
}

fn assert_no_owned_staging(root: &Path) {
    assert_eq!(fs::read_dir(root.join("libs/staging")).unwrap().count(), 0);
}

fn assert_single_installation(root: &Path) {
    let mut names: Vec<_> = fs::read_dir(root.join("libs"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    names.sort();
    assert_eq!(names, ["runtime", "staging", "uv-cache", "uv-python"]);
    assert_no_owned_staging(root);
}

fn assert_installed_plan(root: &Path, paths: &RuntimePaths, plan: &Value) {
    let installed = runtime_dir(root);
    assert_eq!(paths.binary_path.parent().unwrap(), installed.join("bin"));
    assert!(paths.python_path.starts_with(installed.join("python")));
    assert_eq!(paths.worker_path, installed.join("worker/ytdlp_worker.py"));
    assert!(!paths.python_path_entries.is_empty());
    assert!(paths
        .python_path_entries
        .iter()
        .all(|path| path.starts_with(installed.join("python"))));
    let manifest: Value =
        serde_json::from_slice(&fs::read(installed.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["runtime_id"], paths.runtime_id);
    assert_eq!(
        manifest["platform"],
        json!({"os": std::env::consts::OS, "arch": std::env::consts::ARCH})
    );
    assert_eq!(manifest["schema_version"], 2);
    assert_eq!(manifest["plan"]["yt_dlp_version"], plan["yt_dlp"]);
    assert_eq!(manifest["plan"]["environment"]["implementation"], "CPython");
    assert_eq!(
        manifest["plan"]["environment"]["yt_dlp_module_version"],
        plan["yt_dlp"]
    );
    assert!(!Path::new(
        manifest["plan"]["environment"]["base_executable"]
            .as_str()
            .unwrap()
    )
    .starts_with(root));
    let worker = include_bytes!("../worker/ytdlp_worker.py");
    assert_eq!(fs::read(&paths.worker_path).unwrap(), worker);
    assert_eq!(manifest["worker_script_sha256"], sha256_hex(worker));
    for (index, artifact) in plan["artifacts"].as_array().unwrap().iter().enumerate() {
        let name = artifact["name"].as_str().unwrap();
        let actual = &manifest["artifacts"][index];
        for key in ["name", "sha256"] {
            assert_eq!(actual[key], artifact[key]);
        }
        assert_eq!(actual["kind"], ["binary", "wheel"][index]);
        assert_eq!(actual["source"], ["github-release", "pypi"][index]);
        let resolved_tag = plan["yt_dlp"].clone();
        assert_eq!(actual["resolved_tag"], resolved_tag);
        assert_eq!(
            file_sha256(&installed.join("artifacts").join(name)),
            artifact["sha256"]
        );
    }
    assert_eq!(
        file_sha256(&paths.binary_path),
        plan["artifacts"][0]["sha256"]
    );
    assert!(!installed.join("old-only").exists());
    assert_single_installation(root);
}

#[tokio::test]
async fn install_happy_path_then_reuse_without_fetching() {
    let root = TempDir::new().expect("tempdir");
    let fixture = build_fixtures(root.path());
    let plan = write_plan(
        root.path(),
        &plan_json(&fixture, Some(&file_sha256(&fixture.binary)), false),
    );
    let config = config(root.path(), &plan);

    let paths = ensure_runtime(config.clone()).await.expect("first install");
    assert!(paths.binary_path.is_file());
    assert!(paths.python_path.is_file());
    assert!(paths.worker_path.is_file());
    let installed = runtime_dir(root.path());
    assert!(paths.binary_path.starts_with(&installed));
    assert!(paths.python_path.starts_with(&installed));
    assert!(paths.worker_path.starts_with(&installed));
    assert!(installed.join("manifest.json").is_file());
    assert!(installed.join("artifacts").is_dir());
    assert!(installed.join("python/pyvenv.cfg").is_file());
    assert_single_installation(root.path());
    let before = snapshot(&root.path().join("libs"));
    fs::rename(
        root.path().join("fixtures"),
        root.path().join("unavailable-fixtures"),
    )
    .unwrap();
    let mut endpoints: Value = serde_json::from_slice(&fs::read(&plan).unwrap()).unwrap();
    for artifact in endpoints["artifacts"].as_array_mut().unwrap() {
        artifact["url"] = json!(file_url(&root.path().join("different-missing-endpoint")));
    }
    write_plan(root.path(), &endpoints.to_string());

    let reused = ensure_runtime(config)
        .await
        .expect("second install must reuse");
    assert_eq!(paths, reused);
    assert_eq!(
        before,
        snapshot(&root.path().join("libs")),
        "reuse must not create staging or rewrite installed files"
    );
}

#[tokio::test]
async fn equal_explicit_worker_version_installs_and_reuses() {
    let root = TempDir::new().expect("temp root");
    let fixture = build_fixtures(root.path());
    let plan = write_plan(
        root.path(),
        &plan_json(&fixture, Some(&file_sha256(&fixture.binary)), false),
    );
    let mut request = config(root.path(), &plan);
    request.worker_yt_dlp_version = Some(request.yt_dlp_version.clone());
    let paths = ensure_runtime(request.clone()).await.unwrap();
    assert_eq!(ensure_runtime(request).await.unwrap(), paths);
    assert_single_installation(root.path());
}

#[tokio::test]
async fn real_uv_incompatible_python_or_missing_dependency_preserves_prior_runtime() {
    for incompatible_python in [true, false] {
        let root = TempDir::new().unwrap();
        let fixture = build_fixtures(root.path());
        let plan = write_plan(
            root.path(),
            &plan_json(&fixture, Some(&file_sha256(&fixture.binary)), false),
        );
        let request = config(root.path(), &plan);
        ensure_runtime(request.clone()).await.unwrap();
        let before = snapshot(&runtime_dir(root.path()));
        managed::wheel(
            &fixture.wheel,
            VERSION,
            if incompatible_python {
                ">=99"
            } else {
                ">=3.10"
            },
            if incompatible_python {
                ""
            } else {
                "Requires-Dist: missing-wrapper-fixture==1.0\n"
            },
        );
        write_plan(
            root.path(),
            &plan_json(&fixture, Some(&file_sha256(&fixture.binary)), false),
        );
        let error = ensure_runtime(request).await.unwrap_err();
        assert_eq!(error.code(), "RUNTIME_ENVIRONMENT_FAILURE");
        assert!(error.to_string().contains("--runtime-python"));
        assert_eq!(before, snapshot(&runtime_dir(root.path())));
        assert_no_owned_staging(root.path());
    }
}

#[tokio::test]
async fn real_uv_resolves_dependencies_ignores_extras_and_records_distribution() {
    use std::io::Write;
    let root = TempDir::new().unwrap();
    let fixture = build_fixtures(root.path());
    managed::wheel(
        &fixture.wheel,
        VERSION,
        ">=3.10",
        "Requires-Dist: wrapper-fixture-dep==1.0\nProvides-Extra: optional\nRequires-Dist: absent-optional-fixture; extra == 'optional'\n",
    );
    let wheelhouse = fixture.wheel.parent().unwrap();
    let mut dependency = zip::ZipWriter::new(
        fs::File::create(wheelhouse.join("wrapper_fixture_dep-1.0-py3-none-any.whl")).unwrap(),
    );
    for (name, contents) in [
        ("wrapper_fixture_dep.py", "VALUE = 1\n"),
        ("wrapper_fixture_dep-1.0.dist-info/METADATA", "Metadata-Version: 2.3\nName: wrapper-fixture-dep\nVersion: 1.0\n"),
        ("wrapper_fixture_dep-1.0.dist-info/WHEEL", "Wheel-Version: 1.0\nRoot-Is-Purelib: true\nTag: py3-none-any\n"),
        ("wrapper_fixture_dep-1.0.dist-info/RECORD", "wrapper_fixture_dep.py,,\nwrapper_fixture_dep-1.0.dist-info/METADATA,,\nwrapper_fixture_dep-1.0.dist-info/WHEEL,,\nwrapper_fixture_dep-1.0.dist-info/RECORD,,\n"),
    ] {
        dependency.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        dependency.write_all(contents.as_bytes()).unwrap();
    }
    dependency.finish().unwrap();
    let mut source_plan: Value = serde_json::from_str(&plan_json(
        &fixture,
        Some(&file_sha256(&fixture.binary)),
        false,
    ))
    .unwrap();
    source_plan["wheelhouse"] = json!(wheelhouse);
    let plan = write_plan(root.path(), &source_plan.to_string());
    let request = config(root.path(), &plan);
    let first = ensure_runtime(request.clone()).await.unwrap();
    let manifest: Value =
        serde_json::from_slice(&fs::read(runtime_dir(root.path()).join("manifest.json")).unwrap())
            .unwrap();
    let environment = &manifest["plan"]["environment"];
    assert_eq!(environment["yt_dlp_module_version"], VERSION);
    assert!(environment["yt_dlp_distribution_version"].is_string());
    assert_eq!(environment["packages"].as_array().unwrap().len(), 2);
    assert!(environment["packages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|package| package["name"] == "wrapper-fixture-dep" && package["version"] == "1.0"));
    // The relocated environment works after staging deletion and cache removal
    // is unnecessary: a missing tool proves reuse performs no uv resolution.
    let mut reuse = request;
    reuse.uv_path = root.path().join("missing-uv");
    assert_eq!(first, ensure_runtime(reuse).await.unwrap());
}

#[tokio::test]
async fn legacy_plan_and_release_input_are_rejected_with_migration_guidance() {
    let root = TempDir::new().unwrap();
    let fixture = build_fixtures(root.path());
    let mut value: Value = serde_json::from_str(&plan_json(
        &fixture,
        Some(&file_sha256(&fixture.binary)),
        false,
    ))
    .unwrap();
    value["schema_version"] = json!(1);
    let plan = write_plan(root.path(), &value.to_string());
    let error = ensure_runtime(config(root.path(), &plan))
        .await
        .unwrap_err();
    assert_eq!(error.code(), "RUNTIME_CONFIG_INVALID");
    assert!(error.to_string().contains("schema_version 2"));
    let mut request = config(root.path(), &plan);
    request.python_release_tag = Some("20261026".into());
    let error = ensure_runtime(request).await.unwrap_err();
    assert_eq!(error.code(), "RUNTIME_CONFIG_INVALID");
    assert!(error.to_string().contains("--runtime-python"));
    assert!(!runtime_dir(root.path()).exists());
}

#[tokio::test]
async fn real_uv_rejects_unsafe_wheel_paths_before_publication() {
    use std::io::Write;
    let root = TempDir::new().unwrap();
    let fixture = build_fixtures(root.path());
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&fixture.wheel)
        .unwrap();
    let mut archive = zip::ZipWriter::new_append(file).unwrap();
    archive
        .start_file("../outside", zip::write::SimpleFileOptions::default())
        .unwrap();
    archive.write_all(b"not allowed").unwrap();
    archive.finish().unwrap();
    let plan = write_plan(
        root.path(),
        &plan_json(&fixture, Some(&file_sha256(&fixture.binary)), false),
    );
    let error = ensure_runtime(config(root.path(), &plan))
        .await
        .unwrap_err();
    assert_eq!(error.code(), "RUNTIME_ENVIRONMENT_FAILURE");
    assert!(!runtime_dir(root.path()).exists());
    assert!(!root.path().join("outside").exists());
    assert_no_owned_staging(root.path());
}

#[tokio::test]
async fn deflated_wheel_entries_are_extracted() {
    let root = TempDir::new().expect("tempdir");
    let fixture = build_fixtures(root.path());
    let plan = write_plan(
        root.path(),
        &plan_json(&fixture, Some(&file_sha256(&fixture.binary)), false),
    );
    ensure_runtime(config(root.path(), &plan))
        .await
        .expect("install");
    let manifest: Value =
        serde_json::from_slice(&fs::read(runtime_dir(root.path()).join("manifest.json")).unwrap())
            .unwrap();
    assert_eq!(
        manifest["plan"]["environment"]["yt_dlp_module_version"],
        VERSION
    );
}

#[tokio::test]
async fn checksum_mismatch_fails_closed_without_publishing() {
    let root = TempDir::new().expect("tempdir");
    let fixture = build_fixtures(root.path());
    let plan = write_plan(
        root.path(),
        &plan_json(&fixture, Some(&"0".repeat(64)), false),
    );
    let error = ensure_runtime(config(root.path(), &plan))
        .await
        .expect_err("checksum mismatch must fail");
    assert_eq!(error.code(), "RUNTIME_INTEGRITY_FAILURE");
    assert!(
        !runtime_dir(root.path()).exists(),
        "no runtime may be published on failure"
    );
    assert_no_owned_staging(root.path());
}

#[tokio::test]
async fn missing_local_source_fails_before_admission_and_recovers() {
    let root = TempDir::new().expect("tempdir");
    let fixture = build_fixtures(root.path());
    let bad_plan = write_plan(
        root.path(),
        &plan_json(&fixture, Some(&file_sha256(&fixture.binary)), true),
    );
    let error = ensure_runtime(config(root.path(), &bad_plan))
        .await
        .expect_err("missing source must fail");
    assert_eq!(error.code(), "RUNTIME_DOWNLOAD_FAILURE");
    assert!(error.to_string().contains("attempt=3"));
    assert!(!runtime_dir(root.path()).exists());
    assert_no_owned_staging(root.path());

    let good_plan = write_plan(
        root.path(),
        &plan_json(&fixture, Some(&file_sha256(&fixture.binary)), false),
    );
    ensure_runtime(config(root.path(), &good_plan))
        .await
        .expect("corrected plan must install");
    assert_single_installation(root.path());
}

#[tokio::test]
async fn identity_change_replaces_same_path() {
    for change in ["yt-dlp", "python-request", "digest"] {
        let root = TempDir::new().unwrap();
        let fixture = build_fixtures(root.path());
        let mut value: Value = serde_json::from_str(&plan_json(
            &fixture,
            Some(&file_sha256(&fixture.binary)),
            false,
        ))
        .unwrap();
        let plan = write_plan(root.path(), &value.to_string());
        let mut request = config(root.path(), &plan);
        let first = ensure_runtime(request.clone()).await.unwrap();
        fs::write(runtime_dir(root.path()).join("old-only"), b"old").unwrap();
        match change {
            "yt-dlp" => {
                let version = "2026.10.26";
                request.yt_dlp_version = version.into();
                request.worker_yt_dlp_version = Some(version.into());
                value["yt_dlp"] = json!(version);
                value["artifacts"][1]["name"] = json!(format!("yt_dlp-{version}-py3-none-any.whl"));
                write_wheel_version(&fixture.wheel, version);
                value["artifacts"][1]["sha256"] = json!(file_sha256(&fixture.wheel));
            }
            "python-request" => {
                // Same executable, new explicit request text is still an update.
                request.python_request = Some(
                    PathBuf::from(managed::host_python())
                        .parent()
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                );
            }
            "digest" => {
                fs::write(&fixture.binary, b"new pinned binary").unwrap();
                value["artifacts"][0]["sha256"] = json!(file_sha256(&fixture.binary));
            }
            _ => unreachable!(),
        }
        write_plan(root.path(), &value.to_string());
        let second = ensure_runtime(request).await.unwrap();
        assert_ne!(first.runtime_id, second.runtime_id, "{change}");
        assert_eq!(first.binary_path, second.binary_path);
        assert_eq!(first.python_path, second.python_path);
        assert_eq!(first.worker_path, second.worker_path);
        assert_eq!(first.python_path_entries, second.python_path_entries);
        assert_installed_plan(root.path(), &second, &value);
    }
}

#[tokio::test]
async fn retained_archive_binary_or_worker_corruption_reinstalls() {
    for corrupted in ["archive", "binary", "worker"] {
        let root = TempDir::new().unwrap();
        let fixture = build_fixtures(root.path());
        let value: Value = serde_json::from_str(&plan_json(
            &fixture,
            Some(&file_sha256(&fixture.binary)),
            false,
        ))
        .unwrap();
        let plan = write_plan(root.path(), &value.to_string());
        let first = ensure_runtime(config(root.path(), &plan)).await.unwrap();
        let damaged = match corrupted {
            "archive" => runtime_dir(root.path())
                .join("artifacts")
                .join(wheel_name()),
            "binary" => first.binary_path.clone(),
            "worker" => first.worker_path.clone(),
            _ => unreachable!(),
        };
        fs::write(damaged, b"corrupt bytes").unwrap();
        fs::write(runtime_dir(root.path()).join("old-only"), b"old").unwrap();
        let repaired = ensure_runtime(config(root.path(), &plan)).await.unwrap();
        assert_eq!(first, repaired);
        assert_installed_plan(root.path(), &repaired, &value);
    }
}

#[tokio::test]
async fn mismatched_manifest_reinstalls_independent_of_runtime_id() {
    for field in [
        "/runtime_id",
        "/platform/os",
        "/platform/arch",
        "/plan/environment/python_version",
        "/plan/environment/base_executable",
        "/artifacts/1/sha256",
        "/worker_script_sha256",
    ] {
        let root = TempDir::new().unwrap();
        let fixture = build_fixtures(root.path());
        let value: Value = serde_json::from_str(&plan_json(
            &fixture,
            Some(&file_sha256(&fixture.binary)),
            false,
        ))
        .unwrap();
        let plan = write_plan(root.path(), &value.to_string());
        let first = ensure_runtime(config(root.path(), &plan)).await.unwrap();
        let manifest_path = runtime_dir(root.path()).join("manifest.json");
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        *manifest.pointer_mut(field).unwrap() = json!("incorrect");
        if field != "/runtime_id" {
            assert_eq!(manifest["runtime_id"], first.runtime_id);
        }
        fs::write(manifest_path, manifest.to_string()).unwrap();
        fs::write(runtime_dir(root.path()).join("old-only"), b"old").unwrap();
        let repaired = ensure_runtime(config(root.path(), &plan)).await.unwrap();
        assert_eq!(first, repaired);
        assert_installed_plan(root.path(), &repaired, &value);
    }
}

#[tokio::test]
async fn missing_manifest_reinstalls() {
    let root = TempDir::new().unwrap();
    let fixture = build_fixtures(root.path());
    let plan = write_plan(
        root.path(),
        &plan_json(&fixture, Some(&file_sha256(&fixture.binary)), false),
    );
    let first = ensure_runtime(config(root.path(), &plan)).await.unwrap();
    let installed = runtime_dir(root.path());
    fs::remove_file(installed.join("manifest.json")).unwrap();
    fs::write(installed.join("old-only"), b"old").unwrap();
    assert_eq!(
        first,
        ensure_runtime(config(root.path(), &plan)).await.unwrap()
    );
    assert!(installed.join("manifest.json").is_file());
    assert!(!installed.join("old-only").exists());
    assert_single_installation(root.path());
}

#[tokio::test]
async fn missing_required_python_path_reinstalls() {
    let root = TempDir::new().unwrap();
    let fixture = build_fixtures(root.path());
    let plan = write_plan(
        root.path(),
        &plan_json(&fixture, Some(&file_sha256(&fixture.binary)), false),
    );
    let first = ensure_runtime(config(root.path(), &plan)).await.unwrap();
    fs::remove_file(&first.python_path).unwrap();
    fs::write(runtime_dir(root.path()).join("old-only"), b"old").unwrap();
    assert_eq!(
        first,
        ensure_runtime(config(root.path(), &plan)).await.unwrap()
    );
    assert!(first.python_path.is_file());
    assert!(!runtime_dir(root.path()).join("old-only").exists());
    assert_single_installation(root.path());
}

#[tokio::test]
async fn failed_preparation_preserves_prior_runtime() {
    for corrupt in [false, true] {
        for failure in ["source", "hash", "wheel"] {
            let root = TempDir::new().unwrap();
            let fixture = build_fixtures(root.path());
            let plan = write_plan(
                root.path(),
                &plan_json(&fixture, Some(&file_sha256(&fixture.binary)), false),
            );
            let first = ensure_runtime(config(root.path(), &plan)).await.unwrap();
            if corrupt {
                fs::write(&first.worker_path, b"corrupt worker").unwrap();
            } else {
                let manifest_path = runtime_dir(root.path()).join("manifest.json");
                let mut manifest: Value =
                    serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
                manifest["platform"]["arch"] = json!("mismatch");
                fs::write(manifest_path, manifest.to_string()).unwrap();
            }
            let before = snapshot(&runtime_dir(root.path()));
            let expected_code = match failure {
                "source" => {
                    fs::remove_file(&fixture.wheel).unwrap();
                    "RUNTIME_DOWNLOAD_FAILURE"
                }
                "hash" => {
                    fs::write(&fixture.binary, b"source no longer matches its pin").unwrap();
                    "RUNTIME_INTEGRITY_FAILURE"
                }
                "wheel" => {
                    fs::write(&fixture.wheel, b"hash-pinned but not an archive").unwrap();
                    write_plan(
                        root.path(),
                        &plan_json(&fixture, Some(&file_sha256(&fixture.binary)), false),
                    );
                    "RUNTIME_ENVIRONMENT_FAILURE"
                }
                _ => unreachable!(),
            };
            let error = ensure_runtime(config(root.path(), &plan))
                .await
                .unwrap_err();
            assert_eq!(
                error.code(),
                expected_code,
                "corrupt={corrupt} failure={failure}"
            );
            assert!(error.to_string().contains(if failure == "source" {
                "attempt=3"
            } else {
                "attempt=1"
            }));
            assert_eq!(before, snapshot(&runtime_dir(root.path())));
            assert_no_owned_staging(root.path());
        }
    }
}

#[tokio::test]
async fn initial_hash_and_extraction_failures_leave_no_runtime_then_recover() {
    for failure in ["hash", "wheel"] {
        let root = TempDir::new().unwrap();
        let fixture = build_fixtures(root.path());
        let damaged = match failure {
            "hash" => &fixture.binary,
            _ => &fixture.wheel,
        };
        let original = fs::read(damaged).unwrap();
        let original_plan = plan_json(&fixture, Some(&file_sha256(&fixture.binary)), false);
        fs::write(damaged, b"invalid source").unwrap();
        let bad_plan = if failure == "hash" {
            original_plan.clone()
        } else {
            plan_json(&fixture, Some(&file_sha256(&fixture.binary)), false)
        };
        let plan = write_plan(root.path(), &bad_plan);
        let error = ensure_runtime(config(root.path(), &plan))
            .await
            .unwrap_err();
        assert_eq!(
            error.code(),
            if failure == "hash" {
                "RUNTIME_INTEGRITY_FAILURE"
            } else {
                "RUNTIME_ENVIRONMENT_FAILURE"
            }
        );
        assert!(!runtime_dir(root.path()).exists());
        assert_no_owned_staging(root.path());
        fs::write(damaged, original).unwrap();
        write_plan(root.path(), &original_plan);
        let paths = ensure_runtime(config(root.path(), &plan)).await.unwrap();
        assert_installed_plan(
            root.path(),
            &paths,
            &serde_json::from_str(&original_plan).unwrap(),
        );
    }
}

#[tokio::test]
async fn restart_states_ignore_foreign_partial_staging() {
    for state in ["old", "manifest-free", "absent", "published"] {
        let root = TempDir::new().unwrap();
        let fixture = build_fixtures(root.path());
        let value: Value = serde_json::from_str(&plan_json(
            &fixture,
            Some(&file_sha256(&fixture.binary)),
            false,
        ))
        .unwrap();
        let plan = write_plan(root.path(), &value.to_string());
        let request = config(root.path(), &plan);
        if state != "absent" {
            let paths = ensure_runtime(request.clone()).await.unwrap();
            if state == "manifest-free" {
                fs::remove_file(runtime_dir(root.path()).join("manifest.json")).unwrap();
                fs::remove_file(paths.python_path).unwrap();
            } else if state == "old" {
                fs::write(paths.worker_path, b"old build").unwrap();
            }
        }
        let foreign = root.path().join("libs/staging/crashed-boot");
        fs::create_dir_all(&foreign).unwrap();
        fs::write(foreign.join("partial"), b"not ours to reclaim").unwrap();
        let before = snapshot(&foreign);
        let installed_before = (state == "published").then(|| snapshot(&runtime_dir(root.path())));
        if state == "published" {
            fs::rename(
                root.path().join("fixtures"),
                root.path().join("sources-offline"),
            )
            .unwrap();
        }
        let paths = ensure_runtime(request).await.unwrap();
        assert!(paths.worker_path.starts_with(runtime_dir(root.path())));
        assert_eq!(before, snapshot(&foreign));
        assert_eq!(
            fs::read_dir(root.path().join("libs/staging"))
                .unwrap()
                .count(),
            1
        );
        if let Some(before) = installed_before {
            assert_eq!(before, snapshot(&runtime_dir(root.path())));
        }
        assert!(!root.path().join("libs/versions").exists());
    }
}

#[cfg(windows)]
#[tokio::test]
async fn native_windows_locks_stop_manifest_invalidation_or_retired_removal() {
    use std::os::windows::fs::OpenOptionsExt;
    for lock_manifest in [true, false] {
        let root = TempDir::new().unwrap();
        let fixture = build_fixtures(root.path());
        let plan = write_plan(
            root.path(),
            &plan_json(&fixture, Some(&file_sha256(&fixture.binary)), false),
        );
        ensure_runtime(config(root.path(), &plan)).await.unwrap();
        let installed = runtime_dir(root.path());
        fs::write(installed.join("old-only"), b"locked old content").unwrap();
        let locked = File::options()
            .read(true)
            .share_mode(1)
            .open(installed.join(if lock_manifest {
                "manifest.json"
            } else {
                "old-only"
            }))
            .unwrap();
        let before = snapshot(&installed);
        fs::write(&fixture.binary, b"new binary").unwrap();
        write_plan(
            root.path(),
            &plan_json(&fixture, Some(&file_sha256(&fixture.binary)), false),
        );
        let error = ensure_runtime(config(root.path(), &plan))
            .await
            .unwrap_err();
        assert_eq!(error.code(), "RUNTIME_PUBLISH_FAILURE");
        assert!(error.to_string().contains("attempt=1"));
        if lock_manifest {
            assert!(error.to_string().contains("phase=invalidate-manifest"));
            assert_eq!(before, snapshot(&installed));
        } else {
            assert!(error.to_string().contains("phase=remove-retired"));
            assert!(!installed.join("manifest.json").exists());
            assert!(installed.join("old-only").exists());
        }
        assert_no_owned_staging(root.path());
        drop(locked);
        ensure_runtime(config(root.path(), &plan)).await.unwrap();
        assert!(!installed.join("old-only").exists());
        assert_single_installation(root.path());
    }
}

#[tokio::test]
async fn failed_staging_without_manifest_is_cleaned_and_foreign_staging_is_untouched() {
    let root = TempDir::new().unwrap();
    let fixture = build_fixtures(root.path());
    let plan = write_plan(
        root.path(),
        &plan_json(&fixture, Some(&"0".repeat(64)), false),
    );
    let staging = root.path().join("libs/staging");
    for name in ["marked", "unmarked", "crashed-complete"] {
        let foreign = staging.join(name);
        fs::create_dir_all(&foreign).unwrap();
        fs::write(foreign.join("partial-download"), b"not ours").unwrap();
        if name != "unmarked" {
            fs::write(foreign.join("RUNTIME_STAGING_OWNER"), b"owner").unwrap();
        }
        if name == "crashed-complete" {
            fs::write(foreign.join("manifest.json"), b"{}").unwrap();
        }
    }
    let before: Vec<_> = ["marked", "unmarked", "crashed-complete"]
        .map(|name| snapshot(&staging.join(name)))
        .into();
    let error = ensure_runtime(config(root.path(), &plan))
        .await
        .unwrap_err();
    assert_eq!(error.code(), "RUNTIME_INTEGRITY_FAILURE");
    assert!(!runtime_dir(root.path()).exists());
    assert_eq!(fs::read_dir(&staging).unwrap().count(), 3);
    let after: Vec<_> = ["marked", "unmarked", "crashed-complete"]
        .map(|name| snapshot(&staging.join(name)))
        .into();
    assert_eq!(before, after);
}

/// Section 11 allows package-scoped cargo tree or full cargo metadata; tree
/// is not a wider graph. Offline use requires cached Cargo prerequisites.
/// This checks the current graph/files, not the owner's pending delivery commit.
#[test]
fn dependency_graph_has_no_yt_dlp_or_lofty() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let tree = Command::new(env!("CARGO"))
        .current_dir(&manifest_dir)
        .args(["tree", "-p", "ytdlp-http-wrapper", "--offline", "--locked"])
        .output()
        .expect("run cargo tree");
    assert!(
        tree.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&tree.stderr)
    );
    let tree = String::from_utf8(tree.stdout).expect("cargo tree output is UTF-8");
    assert!(
        !tree.trim().is_empty(),
        "cargo tree must report the dependency graph"
    );
    for crate_name in ["yt-dlp", "lofty"] {
        assert!(
            !tree.contains(&format!("{crate_name} v")),
            "{crate_name} must not appear in the dependency graph:\n{tree}"
        );
    }

    let manifest = fs::read_to_string(manifest_dir.join("Cargo.toml")).expect("read Cargo.toml");
    assert!(
        !manifest.contains("[patch.crates-io]"),
        "the lofty patch block must be removed from Cargo.toml"
    );
    assert!(
        !manifest.contains("yt-dlp"),
        "the yt-dlp crate must not be declared in Cargo.toml"
    );
    let lock = fs::read_to_string(manifest_dir.join("Cargo.lock")).expect("read Cargo.lock");
    for crate_name in ["yt-dlp", "lofty"] {
        assert!(
            !lock.contains(&format!("name = \"{crate_name}\"")),
            "{crate_name} must not appear in the resolved Cargo.lock"
        );
    }
}
