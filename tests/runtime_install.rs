#![allow(clippy::expect_used, clippy::unwrap_used)]

//! Deterministic, offline acceptance tests for the project-owned immutable
//! runtime installer. The plan file points at file:// fixtures, so no test
//! touches the network.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use ytdlp_http_wrapper::runtime_installer::{ensure_runtime, RuntimeConfig, RuntimePaths};

const VERSION: &str = "2026.09.23";
const PYTHON_VERSION: &str = "3.13.18";
const PYTHON_RELEASE_TAG: &str = "20261026";

struct Fixture {
    binary: PathBuf,
    python: PathBuf,
    wheel: PathBuf,
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn file_sha256(path: &Path) -> String {
    sha256_hex(&fs::read(path).expect("fixture must be readable"))
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

fn pbs_target() -> &'static str {
    if cfg!(windows) && cfg!(target_arch = "x86_64") {
        "x86_64-pc-windows-msvc"
    } else if cfg!(windows) {
        "i686-pc-windows-msvc"
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        "aarch64-apple-darwin"
    } else if cfg!(target_os = "macos") {
        "x86_64-apple-darwin"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64-unknown-linux-gnu"
    } else if cfg!(target_arch = "arm") {
        "armv7-unknown-linux-gnueabihf"
    } else {
        "x86_64-unknown-linux-gnu"
    }
}

fn python_name() -> String {
    format!(
        "cpython-{}+{}-{}-install_only.tar.gz",
        PYTHON_VERSION,
        PYTHON_RELEASE_TAG,
        pbs_target()
    )
}

fn wheel_name() -> String {
    format!("yt_dlp-{}-py3-none-any.whl", VERSION)
}

fn file_url(path: &Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    format!("file:///{}", path.trim_start_matches('/'))
}

fn build_fixtures(root: &Path) -> Fixture {
    let fixtures = root.join("fixtures");
    fs::create_dir_all(&fixtures).expect("create fixtures");

    let binary = fixtures.join(binary_name());
    fs::write(&binary, b"fake yt-dlp binary\n").expect("write binary fixture");

    let python = fixtures.join(python_name());
    write_python_archive(&python, root);

    let wheel = fixtures.join(wheel_name());
    write_wheel(&wheel);

    Fixture {
        binary,
        python,
        wheel,
    }
}

fn write_python_archive(path: &Path, root: &Path) {
    let tar_root = root.join("python-tree");
    fs::create_dir_all(tar_root.join("python").join("bin")).expect("create python fixture tree");
    fs::write(
        tar_root.join("python").join("bin").join("python3"),
        b"python marker\n",
    )
    .expect("write python3 marker");
    fs::write(
        tar_root.join("python").join("python.exe"),
        b"python marker\n",
    )
    .expect("write python.exe marker");
    let status = Command::new("tar")
        .arg("-czf")
        .arg(path)
        .arg("-C")
        .arg(&tar_root)
        .arg("python")
        .status()
        .expect("system tar must be available for the fixture");
    assert!(status.success(), "tar failed to build {path:?}");
}

fn write_wheel(path: &Path) {
    write_wheel_version(path, VERSION);
}

fn write_wheel_version(path: &Path, version: &str) {
    let file = File::create(path).expect("create wheel fixture");
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    writer
        .start_file("yt_dlp/__init__.py", options)
        .expect("start __init__.py");
    writer
        .write_all(format!("__version__ = \"{version}\"\n").as_bytes())
        .expect("write __init__.py");
    writer
        .start_file("yt_dlp/version.py", options)
        .expect("start version.py");
    writer
        .write_all(format!("__version__ = \"{version}\"\n").as_bytes())
        .expect("write version.py");
    writer
        .start_file(format!("yt_dlp-{version}.dist-info/RECORD"), options)
        .expect("start RECORD");
    writer.write_all(b"").expect("write RECORD");
    writer.finish().expect("finish wheel fixture");
}

fn plan_json(fixture: &Fixture, binary_sha: Option<&str>, missing_python: bool) -> String {
    let python_url = if missing_python {
        file_url(&fixture.python.with_extension("missing.tar.gz"))
    } else {
        file_url(&fixture.python)
    };
    json!({
        "schema_version": 1,
        "yt_dlp": VERSION,
        "python": {
            "release_tag": PYTHON_RELEASE_TAG,
            "python_version": PYTHON_VERSION
        },
        "artifacts": [
            {
                "name": binary_name(),
                "url": file_url(&fixture.binary),
                "sha256": binary_sha.unwrap_or("").to_owned()
            },
            {
                "name": python_name(),
                "url": python_url,
                "sha256": file_sha256(&fixture.python)
            },
            {
                "name": wheel_name(),
                "url": file_url(&fixture.wheel),
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
        python_release_tag: PYTHON_RELEASE_TAG.to_owned(),
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
        assert!(
            !metadata.file_type().is_symlink(),
            "fixture snapshot expects ordinary paths"
        );
        entries.push(SnapshotEntry {
            path: path.strip_prefix(root).unwrap().to_owned(),
            directory: metadata.is_dir(),
            bytes: if metadata.is_dir() {
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
    assert_eq!(names, ["runtime", "staging"]);
    assert_no_owned_staging(root);
}

fn assert_installed_plan(root: &Path, paths: &RuntimePaths, plan: &Value) {
    let installed = runtime_dir(root);
    assert_eq!(paths.binary_path.parent().unwrap(), installed.join("bin"));
    assert!(paths.python_path.starts_with(installed.join("python")));
    assert_eq!(paths.worker_path, installed.join("worker/ytdlp_worker.py"));
    assert_eq!(
        paths.python_path_entries,
        [installed.join("python/site-packages")]
    );
    let manifest: Value =
        serde_json::from_slice(&fs::read(installed.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["runtime_id"], paths.runtime_id);
    assert_eq!(
        manifest["platform"],
        json!({"os": std::env::consts::OS, "arch": std::env::consts::ARCH})
    );
    assert_eq!(
        manifest["versions"],
        json!({
            "yt_dlp": plan["yt_dlp"], "python": plan["python"]["python_version"],
            "python_release_tag": plan["python"]["release_tag"]
        })
    );
    let worker = include_bytes!("../worker/ytdlp_worker.py");
    assert_eq!(fs::read(&paths.worker_path).unwrap(), worker);
    assert_eq!(manifest["worker_script_sha256"], sha256_hex(worker));
    for (index, artifact) in plan["artifacts"].as_array().unwrap().iter().enumerate() {
        let name = artifact["name"].as_str().unwrap();
        let actual = &manifest["artifacts"][index];
        for key in ["name", "sha256"] {
            assert_eq!(actual[key], artifact[key]);
        }
        assert_eq!(actual["kind"], ["binary", "python", "wheel"][index]);
        assert_eq!(
            actual["source"],
            ["github-release", "python-build-standalone", "pypi"][index]
        );
        let resolved_tag = if index == 1 {
            plan["python"]["release_tag"].clone()
        } else {
            plan["yt_dlp"].clone()
        };
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
    assert!(installed
        .join("python")
        .join("site-packages")
        .join("yt_dlp")
        .join("version.py")
        .is_file());
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
    let version_file = runtime_dir(root.path())
        .join("python")
        .join("site-packages")
        .join("yt_dlp")
        .join("version.py");
    let contents = fs::read_to_string(version_file).expect("version.py must be readable");
    assert!(contents.contains(VERSION));
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
    for change in ["yt-dlp", "python-version", "python-release", "digest"] {
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
                value["artifacts"][2]["name"] = json!(format!("yt_dlp-{version}-py3-none-any.whl"));
                write_wheel_version(&fixture.wheel, version);
                value["artifacts"][2]["sha256"] = json!(file_sha256(&fixture.wheel));
            }
            "python-version" => {
                value["python"]["python_version"] = json!("3.13.19");
                value["artifacts"][1]["name"] =
                    json!(python_name().replace(PYTHON_VERSION, "3.13.19"));
            }
            "python-release" => {
                request.python_release_tag = "20261101".into();
                value["python"]["release_tag"] = json!("20261101");
                value["artifacts"][1]["name"] =
                    json!(python_name().replace(PYTHON_RELEASE_TAG, "20261101"));
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
                .join(python_name()),
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
        "/versions/python",
        "/versions/python_release_tag",
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
        for failure in ["source", "hash", "tar", "wheel"] {
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
                "tar" | "wheel" => {
                    fs::write(
                        if failure == "tar" {
                            &fixture.python
                        } else {
                            &fixture.wheel
                        },
                        b"hash-pinned but not an archive",
                    )
                    .unwrap();
                    write_plan(
                        root.path(),
                        &plan_json(&fixture, Some(&file_sha256(&fixture.binary)), false),
                    );
                    "RUNTIME_EXTRACT_FAILURE"
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
    for failure in ["hash", "tar", "wheel"] {
        let root = TempDir::new().unwrap();
        let fixture = build_fixtures(root.path());
        let damaged = match failure {
            "hash" => &fixture.binary,
            "tar" => &fixture.python,
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
                "RUNTIME_EXTRACT_FAILURE"
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
