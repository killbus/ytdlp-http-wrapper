//! Service-level yt-dlp updates, using the existing contained CLI supervisor.
use std::{path::PathBuf, sync::Arc, time::Duration};

use tokio::sync::{oneshot, RwLock};
use tokio::time::{timeout, Instant};

use crate::{
    backend::Backend,
    models::RunResponse,
    routes::AppState,
    runtime_installer::{self, OnlineUpdate, RuntimeConfig, RuntimePaths},
    supervisor::{Failure, Reply},
};

pub struct UpdateConfig {
    pub runtime: RuntimeConfig,
    pub pool_size: usize,
    pub extra_python_paths: Vec<PathBuf>,
    pub unsupported_reason: Option<String>,
}

pub(crate) struct ActiveRuntime {
    pub state: Arc<AppState>,
    paths: RuntimePaths,
    version: String,
}

pub struct OnlineUpdater {
    config: UpdateConfig,
    pub(crate) active: Arc<RwLock<ActiveRuntime>>,
}

struct UpdateGuard {
    state: Arc<AppState>,
    finished: bool,
}

impl Drop for UpdateGuard {
    fn drop(&mut self) {
        if !self.finished {
            self.state.begin_shutdown();
        }
    }
}

impl OnlineUpdater {
    pub(crate) fn new(
        config: UpdateConfig,
        state: Arc<AppState>,
        paths: RuntimePaths,
        version: String,
    ) -> Self {
        Self {
            config,
            active: Arc::new(RwLock::new(ActiveRuntime {
                state,
                paths,
                version,
            })),
        }
    }

    fn backend(
        &self,
        old: &ActiveRuntime,
        paths: &RuntimePaths,
        version: &str,
    ) -> Result<Backend, String> {
        let mut python_paths = paths.python_path_entries.clone();
        python_paths.extend(self.config.extra_python_paths.clone());
        Backend::from_runtime_config(
            old.state.backend.name(),
            paths.python_path.clone(),
            paths.worker_path.clone(),
            python_paths,
            version,
            old.state.temp_dir.clone(),
            self.config.pool_size,
            &paths.runtime_id,
        )
    }

    pub(crate) async fn run(
        &self,
        root: Arc<AppState>,
        duration: Duration,
        mut reply: oneshot::Sender<Reply>,
    ) {
        let mut guard = UpdateGuard {
            state: root.clone(),
            finished: false,
        };
        let result = self.update(root, duration, &mut reply).await;
        guard.finished = true;
        let _ = reply.send(result);
    }

    async fn update(
        &self,
        root: Arc<AppState>,
        duration: Duration,
        reply: &mut oneshot::Sender<Reply>,
    ) -> Reply {
        if let Some(reason) = &self.config.unsupported_reason {
            return Ok(failed(reason));
        }
        let started = Instant::now();
        // A write lease drains running requests; new requests cannot spawn
        // against a runtime being replaced. No global execution permit is held.
        let mut active = tokio::select! {
            result = timeout(duration, self.active.write()) => match result {
                Ok(lease) => lease,
                Err(_) => return Ok(interrupted()),
            },
            _ = root.shutdown.cancelled() => return Ok(interrupted()),
            _ = reply.closed() => return Ok(interrupted()),
        };
        if root.shutdown.is_cancelled() || reply.is_closed() {
            return Ok(interrupted());
        }
        if let Backend::Resident(Some(manager)) = &active.state.backend {
            if let Err(error) = manager.shutdown().await {
                root.begin_shutdown();
                return Err(Failure::new("UPDATE_STOP_FAILURE", error));
            }
        }

        let result = self
            .install(&root, duration.saturating_sub(started.elapsed()), reply)
            .await;
        let update = match result {
            Ok(response) if response.exit_code == 0 => {
                serde_json::from_str::<OnlineUpdate>(&response.stdout)
                    .map_err(|e| failed(&format!("Invalid installer result: {e}")))
            }
            Ok(response) => Err(response),
            Err(error) => {
                // Supervision could not establish quiescence. Do not touch files.
                root.begin_shutdown();
                return Err(error);
            }
        };
        let response = match update {
            Ok(update)
                if !root.shutdown.is_cancelled()
                    && !reply.is_closed()
                    && started.elapsed() < duration =>
            {
                match self.backend(&active, &update.paths, &update.version) {
                    Ok(backend) => match backend.start().await {
                        Ok(())
                            if root.shutdown.is_cancelled()
                                || reply.is_closed()
                                || started.elapsed() >= duration =>
                        {
                            if let Backend::Resident(Some(manager)) = &backend {
                                if let Err(error) = manager.shutdown().await {
                                    root.begin_shutdown();
                                    return Err(Failure::new("UPDATE_STOP_FAILURE", error));
                                }
                            }
                            interrupted()
                        }
                        Ok(()) => {
                            if let Err(error) = runtime_installer::finish_online_update(
                                &self.config.runtime.libs_dir,
                            ) {
                                if let Backend::Resident(Some(manager)) = &backend {
                                    if let Err(stop_error) = manager.shutdown().await {
                                        root.begin_shutdown();
                                        return Err(Failure::new(
                                            "UPDATE_STOP_FAILURE",
                                            stop_error,
                                        ));
                                    }
                                }
                                failed(&error.to_string())
                            } else {
                                let text = if update.previous_version == update.version {
                                    format!("yt-dlp {} is already up to date.\n", update.version)
                                } else {
                                    format!(
                                        "Updated yt-dlp {} -> {}.\n",
                                        update.previous_version, update.version
                                    )
                                };
                                active.state = execution_state(&root, &update.paths, backend);
                                active.paths = update.paths;
                                active.version = update.version;
                                return Ok(RunResponse {
                                    exit_code: 0,
                                    stdout: text,
                                    stderr: String::new(),
                                });
                            }
                        }
                        Err(error) => {
                            // start() can have started earlier slots. Stop them
                            // before restoring paths used by those processes.
                            if let Backend::Resident(Some(manager)) = &backend {
                                if let Err(stop_error) = manager.shutdown().await {
                                    root.begin_shutdown();
                                    return Err(Failure::new("UPDATE_STOP_FAILURE", stop_error));
                                }
                            }
                            failed(&error)
                        }
                    },
                    Err(error) => failed(&error),
                }
            }
            Ok(_) => interrupted(),
            Err(response) => response,
        };

        if let Err(error) = runtime_installer::recover_online_update(&self.config.runtime.libs_dir)
        {
            root.begin_shutdown();
            return Err(Failure::new("UPDATE_ROLLBACK_FAILURE", error));
        }
        if !root.shutdown.is_cancelled() {
            let backend = self
                .backend(&active, &active.paths, &active.version)
                .map_err(|e| Failure::new("UPDATE_RESTART_FAILURE", e));
            match backend {
                Ok(backend) => match backend.start().await {
                    Ok(()) => active.state = execution_state(&root, &active.paths, backend),
                    Err(error) => {
                        backend.shutdown().await;
                        root.begin_shutdown();
                        return Err(Failure::new("UPDATE_RESTART_FAILURE", error));
                    }
                },
                Err(error) => {
                    root.begin_shutdown();
                    return Err(error);
                }
            }
        }
        // Update errors use the same exit-code contract as yt-dlp execution.
        Ok(response)
    }

    async fn install(
        &self,
        root: &Arc<AppState>,
        duration: Duration,
        reply: &mut oneshot::Sender<Reply>,
    ) -> Reply {
        if duration.is_zero() || root.shutdown.is_cancelled() || reply.is_closed() {
            return Ok(interrupted());
        }
        let config = &self.config.runtime;
        let mut args = vec![
            "--prepare-online-update".to_owned(),
            "--libs-dir".to_owned(),
            config.libs_dir.to_string_lossy().into_owned(),
            "--runtime-yt-dlp-version".to_owned(),
            config.yt_dlp_version.clone(),
            "--runtime-uv".to_owned(),
            config.uv_path.to_string_lossy().into_owned(),
        ];
        if let Some(request) = &config.python_request {
            args.extend(["--runtime-python".to_owned(), request.clone()]);
        }
        let tool = std::env::current_exe().map_err(|e| Failure::new("UPDATE_SPAWN_FAILURE", e))?;
        let mut state = AppState::new(tool, 1, root.temp_dir.clone());
        state.shutdown = root.shutdown.clone();
        let permit = state
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| Failure::new("SHUTTING_DOWN", e))?;
        let (sender, mut receiver) = oneshot::channel();
        let task = root.tasks.spawn(crate::supervisor::supervise(
            "runtime-update".to_owned(),
            args,
            duration,
            0,
            Arc::new(state),
            permit,
            sender,
        ));
        let result = tokio::select! {
            result = &mut receiver => result.map_err(|e| Failure::new("UPDATE_SUPERVISOR_FAILURE", e))?,
            _ = reply.closed() => { drop(receiver); Ok(interrupted()) },
        };
        task.await
            .map_err(|e| Failure::new("UPDATE_SUPERVISOR_FAILURE", e))?;
        if root.shutdown.is_cancelled() {
            return Err(Failure::new(
                "UPDATE_INTERRUPTED",
                "service shutdown; update recovery will run at next boot",
            ));
        }
        result
    }
}

fn execution_state(root: &AppState, paths: &RuntimePaths, backend: Backend) -> Arc<AppState> {
    Arc::new(AppState {
        binary_path: paths.binary_path.clone(),
        semaphore: root.semaphore.clone(),
        temp_dir: root.temp_dir.clone(),
        backend,
        updater: None,
        shutdown: root.shutdown.clone(),
        tasks: root.tasks.clone(),
    })
}

fn failed(message: &str) -> RunResponse {
    RunResponse {
        exit_code: 1,
        stdout: String::new(),
        stderr: format!("{message}\n"),
    }
}

fn interrupted() -> RunResponse {
    RunResponse {
        exit_code: -1,
        stdout: String::new(),
        stderr: "yt-dlp update interrupted; no update was acknowledged.\n".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[tokio::test]
    async fn update_timeout_waits_for_request_lease_without_stopping_backend() {
        let root = tempfile::tempdir().unwrap();
        let config = RuntimeConfig {
            libs_dir: root.path().join("uncreated"),
            plan_path: None,
            yt_dlp_version: "2026.10.26".into(),
            python_release_tag: None,
            python_request: None,
            uv_path: "uv".into(),
            worker_yt_dlp_version: None,
        };
        let paths = RuntimePaths {
            binary_path: "unused".into(),
            python_path: "unused".into(),
            worker_path: "unused".into(),
            python_path_entries: vec![],
            runtime_id: "old".into(),
        };
        let state = Arc::new(
            AppState::new(paths.binary_path.clone(), 1, None).with_updates(
                UpdateConfig {
                    runtime: config.clone(),
                    pool_size: 1,
                    extra_python_paths: vec![],
                    unsupported_reason: None,
                },
                paths,
                config.yt_dlp_version.clone(),
            ),
        );
        let updater = state.updater.as_ref().unwrap();
        let lease = updater.active.read().await;
        let (mut sender, _receiver) = oneshot::channel();
        let result = updater
            .update(state.clone(), Duration::from_millis(10), &mut sender)
            .await
            .unwrap_or_else(|error| panic!("{}", error.message));
        assert_eq!(result.exit_code, -1);
        assert_eq!(lease.version, "2026.10.26");
        assert!(!state.shutdown.is_cancelled());
        assert!(!config.libs_dir.exists());
        assert_eq!(state.semaphore.available_permits(), 1);
    }
}
