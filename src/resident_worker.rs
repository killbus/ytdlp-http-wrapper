#[cfg(windows)]
use process_wrap::tokio::JobObject;
#[cfg(unix)]
use process_wrap::tokio::ProcessGroup;
use process_wrap::tokio::{ChildWrapper, CommandWrap, KillOnDrop};
use serde::Deserialize;
use serde_json::json;
use std::{
    fmt,
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::{ChildStderr, ChildStdin, ChildStdout},
    time::{timeout, Duration},
};

use crate::{
    models::RunResponse,
    worker_protocol::{read_frame, write_frame, Frame, MessageType, PROTOCOL_VERSION},
};

const STARTUP_REQUEST_ID: &str = "startup";
const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
const CANCELLATION_GRACE: Duration = Duration::from_millis(500);
const REAP_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_DIAGNOSTIC_BYTES: usize = 10 * 1024 * 1024;
const MAX_RESULT_STREAM_BYTES: usize = 10 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct ResidentWorkerConfig {
    pub python_path: PathBuf,
    pub worker_path: PathBuf,
    pub python_path_entries: Vec<PathBuf>,
    pub generation_id: String,
    pub worker_id: String,
    pub expected_yt_dlp_version: String,
    /// Parent directory for per-request temporary files, mirroring the CLI
    /// backend's `--temp-dir`. `None` uses the system temporary directory.
    pub temp_dir: Option<PathBuf>,
}

#[derive(Debug)]
pub struct ResidentWorkerError {
    code: String,
    message: String,
}

impl ResidentWorkerError {
    fn new(code: impl Into<String>, message: impl fmt::Display) -> Self {
        Self {
            code: code.into(),
            message: message.to_string(),
        }
    }

    pub(crate) fn manager_error(code: impl Into<String>, message: impl fmt::Display) -> Self {
        Self::new(code, message)
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ResidentWorkerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ResidentWorkerError {}

pub struct ResidentWorker {
    child: Box<dyn ChildWrapper>,
    #[cfg(unix)]
    pgid: i32,
    stdin: ChildStdin,
    stdout: ChildStdout,
    stderr: ChildStderr,
    diagnostics: Vec<u8>,
    generation_id: String,
    worker_id: String,
    healthy: bool,
    temp_dir: Option<PathBuf>,
}

impl fmt::Debug for ResidentWorker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResidentWorker")
            .field("process_id", &self.process_id())
            .field("generation_id", &self.generation_id)
            .field("worker_id", &self.worker_id)
            .field("healthy", &self.healthy)
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HelloPayload {
    protocol_version: u32,
    yt_dlp_version: String,
    python_version: String,
    generation_id: String,
    worker_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResultPayload {
    exit_code: i32,
    stdout: String,
    stderr: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ErrorPayload {
    code: String,
    message: String,
}

impl ResidentWorker {
    pub async fn start(config: ResidentWorkerConfig) -> Result<Self, ResidentWorkerError> {
        let python_path = join_python_path(&config.python_path_entries)?;

        let mut command = CommandWrap::with_new(&config.python_path, |command| {
            command
                .arg(&config.worker_path)
                .env("YTDLP_GENERATION_ID", &config.generation_id)
                .env("YTDLP_WORKER_ID", &config.worker_id)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            if let Some(python_path) = python_path.as_ref() {
                command.env("PYTHONPATH", python_path);
            }
        });
        command.wrap(KillOnDrop);
        #[cfg(unix)]
        command.wrap(ProcessGroup::leader());
        #[cfg(windows)]
        command.wrap(JobObject);

        let mut child = command
            .spawn()
            .map_err(|error| ResidentWorkerError::new("WORKER_SPAWN_FAILURE", error))?;
        #[cfg(unix)]
        let pgid = child.id().ok_or_else(|| {
            ResidentWorkerError::new(
                "WORKER_SPAWN_FAILURE",
                "worker process ID was unavailable after spawn",
            )
        })? as i32;
        let stdin = child.stdin().take().ok_or_else(|| {
            ResidentWorkerError::new("WORKER_SPAWN_FAILURE", "worker stdin was not piped")
        })?;
        let stdout = child.stdout().take().ok_or_else(|| {
            ResidentWorkerError::new("WORKER_SPAWN_FAILURE", "worker stdout was not piped")
        })?;
        let stderr = child.stderr().take().ok_or_else(|| {
            ResidentWorkerError::new("WORKER_SPAWN_FAILURE", "worker stderr was not piped")
        })?;

        let mut worker = Self {
            child,
            #[cfg(unix)]
            pgid,
            stdin,
            stdout,
            stderr,
            diagnostics: Vec::new(),
            generation_id: config.generation_id,
            worker_id: config.worker_id,
            healthy: true,
            temp_dir: config.temp_dir,
        };

        let handshake = worker.handshake(&config.expected_yt_dlp_version).await;
        if let Err(error) = handshake {
            return Err(worker.terminate_preserving(error).await);
        }
        Ok(worker)
    }

    pub fn process_id(&self) -> Option<u32> {
        self.child.id()
    }

    pub fn is_healthy(&self) -> bool {
        self.healthy
    }

    pub async fn execute(
        &mut self,
        request_id: &str,
        args: Vec<String>,
    ) -> Result<RunResponse, ResidentWorkerError> {
        self.execute_with_deadline(request_id, args, None).await
    }

    pub async fn execute_with_timeout(
        &mut self,
        request_id: &str,
        args: Vec<String>,
        duration: Duration,
    ) -> Result<RunResponse, ResidentWorkerError> {
        self.execute_with_deadline(request_id, args, Some(duration))
            .await
    }

    async fn execute_with_deadline(
        &mut self,
        request_id: &str,
        args: Vec<String>,
        duration: Option<Duration>,
    ) -> Result<RunResponse, ResidentWorkerError> {
        if !self.healthy {
            return Err(ResidentWorkerError::new(
                "WORKER_UNHEALTHY",
                "worker has been retired and cannot accept another request",
            ));
        }

        // Per-request temp-directory ownership mirrors the CLI supervisor: the
        // controller creates the owned directory before dispatch and removes it
        // explicitly on every settle path (success, timeout, or protocol
        // failure). The worker only borrows the directory for the request.
        let temp = RequestTemp::new(&self.temp_dir)?;
        let request_temp = temp.path().to_path_buf();
        let result = self
            .run_request(request_id, args, duration, &request_temp)
            .await;
        let cleanup = temp.settle();
        match (result, cleanup) {
            (Ok(response), None) => Ok(response),
            (Ok(_), Some(cleanup)) => Err(cleanup),
            (Err(error), None) => Err(error),
            (Err(error), Some(cleanup)) => Err(ResidentWorkerError::new(
                "TEMP_CLEANUP_FAILURE",
                format!("original failure: {error}; {cleanup}"),
            )),
        }
    }

    async fn run_request(
        &mut self,
        request_id: &str,
        args: Vec<String>,
        duration: Option<Duration>,
        request_temp: &Path,
    ) -> Result<RunResponse, ResidentWorkerError> {
        let result = match duration {
            Some(duration) => {
                match timeout(duration, self.execute_inner(request_id, args, request_temp)).await {
                    Ok(result) => result,
                    Err(_) => {
                        self.healthy = false;
                        self.cancel_active_request(request_id).await;
                        let error = ResidentWorkerError::new(
                            "WORKER_TIMEOUT",
                            "resident worker request exceeded its deadline",
                        );
                        return Err(self.terminate_preserving(error).await);
                    }
                }
            }
            None => self.execute_inner(request_id, args, request_temp).await,
        };
        match result {
            Err(error)
                if matches!(
                    error.code(),
                    "WORKER_PROTOCOL_FAILURE" | "WORKER_OUTPUT_LIMIT"
                ) =>
            {
                self.healthy = false;
                Err(self.terminate_preserving(error).await)
            }
            result => result,
        }
    }

    async fn cancel_active_request(&mut self, request_id: &str) {
        let Ok(frame) = cancellation_frame(request_id) else {
            return;
        };
        if write_frame(&mut self.stdin, &frame).await.is_err() {
            return;
        }

        // The current Python adapter is single-threaded, so cancellation is
        // cooperative only when the active request can return to its protocol
        // loop. Bound the grace period and then reclaim the whole process tree.
        let _ = timeout(
            CANCELLATION_GRACE,
            self.read_frame_or_diagnostic("WORKER_PROTOCOL_FAILURE"),
        )
        .await;
    }

    async fn execute_inner(
        &mut self,
        request_id: &str,
        args: Vec<String>,
        request_temp: &Path,
    ) -> Result<RunResponse, ResidentWorkerError> {
        // The run payload carries the controller-owned request directory; the
        // worker only borrows it for the duration of the request.
        let frame = Frame::new(
            MessageType::Run,
            request_id,
            json!({
                "args": args,
                "request_temp": request_temp.to_string_lossy().into_owned(),
            }),
        )
        .map_err(|error| ResidentWorkerError::new("WORKER_PROTOCOL_FAILURE", error))?;
        write_frame(&mut self.stdin, &frame)
            .await
            .map_err(|error| ResidentWorkerError::new("WORKER_PROTOCOL_FAILURE", error))?;

        let response = self
            .read_frame_or_diagnostic("WORKER_PROTOCOL_FAILURE")
            .await?
            .ok_or_else(|| {
                ResidentWorkerError::new(
                    "WORKER_PROTOCOL_FAILURE",
                    "worker closed stdout during request",
                )
            })?;
        response
            .validate_request_id(request_id)
            .map_err(|error| ResidentWorkerError::new("WORKER_PROTOCOL_FAILURE", error))?;

        match response.message_type {
            MessageType::Result => {
                let result: ResultPayload = serde_json::from_value(response.payload)
                    .map_err(|error| ResidentWorkerError::new("WORKER_PROTOCOL_FAILURE", error))?;
                if result.stdout.len() > MAX_RESULT_STREAM_BYTES
                    || result.stderr.len() > MAX_RESULT_STREAM_BYTES
                {
                    return Err(ResidentWorkerError::new(
                        "WORKER_OUTPUT_LIMIT",
                        "worker result exceeded the 10 MiB per-stream limit",
                    ));
                }
                Ok(RunResponse {
                    exit_code: result.exit_code,
                    stdout: result.stdout,
                    stderr: result.stderr,
                })
            }
            MessageType::Error => {
                let error: ErrorPayload = serde_json::from_value(response.payload)
                    .map_err(|error| ResidentWorkerError::new("WORKER_PROTOCOL_FAILURE", error))?;
                // Surface the worker's structured business code verbatim; a
                // business error must not retire the worker.
                Err(ResidentWorkerError::new(error.code, error.message))
            }
            message_type => Err(ResidentWorkerError::new(
                "WORKER_PROTOCOL_FAILURE",
                format!("unexpected response message: {message_type:?}"),
            )),
        }
    }

    pub async fn shutdown(mut self) -> Result<(), ResidentWorkerError> {
        match self.shutdown_inner().await {
            Ok(()) => Ok(()),
            Err(error) => Err(self.terminate_preserving(error).await),
        }
    }

    async fn shutdown_inner(&mut self) -> Result<(), ResidentWorkerError> {
        let request_id = "shutdown";
        let frame = Frame::new(MessageType::Shutdown, request_id, json!({}))
            .map_err(|error| ResidentWorkerError::new("WORKER_PROTOCOL_FAILURE", error))?;
        write_frame(&mut self.stdin, &frame)
            .await
            .map_err(|error| ResidentWorkerError::new("WORKER_PROTOCOL_FAILURE", error))?;

        let response = timeout(
            SHUTDOWN_TIMEOUT,
            self.read_frame_or_diagnostic("WORKER_PROTOCOL_FAILURE"),
        )
        .await
        .map_err(|error| ResidentWorkerError::new("WORKER_SHUTDOWN_FAILURE", error))??
        .ok_or_else(|| {
            ResidentWorkerError::new(
                "WORKER_SHUTDOWN_FAILURE",
                "worker closed stdout before shutdown acknowledgement",
            )
        })?;
        response
            .validate_request_id(request_id)
            .map_err(|error| ResidentWorkerError::new("WORKER_PROTOCOL_FAILURE", error))?;
        if response.message_type != MessageType::Result
            || response.payload != json!({ "shutdown": true })
        {
            return Err(ResidentWorkerError::new(
                "WORKER_SHUTDOWN_FAILURE",
                "worker returned an invalid shutdown acknowledgement",
            ));
        }

        self.stdin
            .shutdown()
            .await
            .map_err(|error| ResidentWorkerError::new("WORKER_SHUTDOWN_FAILURE", error))?;
        let status = timeout(SHUTDOWN_TIMEOUT, self.child.inner_mut().wait())
            .await
            .map_err(|error| ResidentWorkerError::new("WORKER_SHUTDOWN_FAILURE", error))?
            .map_err(|error| ResidentWorkerError::new("WORKER_SHUTDOWN_FAILURE", error))?;
        #[cfg(unix)]
        if timeout(REAP_TIMEOUT, wait_for_group(self.pgid))
            .await
            .is_err()
        {
            return Err(ResidentWorkerError::new(
                "WORKER_SHUTDOWN_FAILURE",
                "worker descendants remained after graceful shutdown",
            ));
        }
        #[cfg(windows)]
        timeout(REAP_TIMEOUT, self.child.wait())
            .await
            .map_err(|error| ResidentWorkerError::new("WORKER_SHUTDOWN_FAILURE", error))?
            .map_err(|error| ResidentWorkerError::new("WORKER_SHUTDOWN_FAILURE", error))?;
        self.capture_remaining_diagnostics()
            .await
            .map_err(|error| ResidentWorkerError::new("WORKER_SHUTDOWN_FAILURE", error))?;
        if !status.success() {
            return Err(ResidentWorkerError::new(
                "WORKER_SHUTDOWN_FAILURE",
                format!(
                    "worker exited with {status}; diagnostics: {}",
                    String::from_utf8_lossy(&self.diagnostics)
                ),
            ));
        }
        if !self.diagnostics.is_empty() {
            return Err(ResidentWorkerError::new(
                "WORKER_PROTOCOL_FAILURE",
                "worker wrote diagnostics during a clean lifecycle",
            ));
        }
        Ok(())
    }

    async fn handshake(
        &mut self,
        expected_yt_dlp_version: &str,
    ) -> Result<(), ResidentWorkerError> {
        let hello = timeout(
            STARTUP_TIMEOUT,
            self.read_frame_or_diagnostic("WORKER_HANDSHAKE_FAILURE"),
        )
        .await
        .map_err(|error| ResidentWorkerError::new("WORKER_HANDSHAKE_FAILURE", error))??
        .ok_or_else(|| {
            ResidentWorkerError::new(
                "WORKER_HANDSHAKE_FAILURE",
                "worker closed stdout before hello",
            )
        })?;
        if hello.message_type != MessageType::Hello {
            return Err(ResidentWorkerError::new(
                "WORKER_HANDSHAKE_FAILURE",
                "worker's first frame was not hello",
            ));
        }
        hello
            .validate_request_id(STARTUP_REQUEST_ID)
            .map_err(|error| ResidentWorkerError::new("WORKER_HANDSHAKE_FAILURE", error))?;
        let payload: HelloPayload = serde_json::from_value(hello.payload)
            .map_err(|error| ResidentWorkerError::new("WORKER_HANDSHAKE_FAILURE", error))?;
        if payload.protocol_version != PROTOCOL_VERSION
            || payload.generation_id != self.generation_id
            || payload.worker_id != self.worker_id
            || payload.yt_dlp_version != expected_yt_dlp_version
            || payload.python_version.is_empty()
        {
            return Err(ResidentWorkerError::new(
                "WORKER_HANDSHAKE_FAILURE",
                "worker identity or runtime metadata did not match configuration",
            ));
        }

        let ready = Frame::new(MessageType::Ready, STARTUP_REQUEST_ID, json!({}))
            .map_err(|error| ResidentWorkerError::new("WORKER_HANDSHAKE_FAILURE", error))?;
        write_frame(&mut self.stdin, &ready)
            .await
            .map_err(|error| ResidentWorkerError::new("WORKER_HANDSHAKE_FAILURE", error))
    }

    async fn read_frame_or_diagnostic(
        &mut self,
        error_code: &'static str,
    ) -> Result<Option<Frame>, ResidentWorkerError> {
        let mut diagnostic = [0_u8; 8192];
        tokio::select! {
            biased;
            result = self.stderr.read(&mut diagnostic) => {
                let size = result
                    .map_err(|error| ResidentWorkerError::new(error_code, error))?;
                if size == 0 {
                    return Err(ResidentWorkerError::new(
                        error_code,
                        "worker closed stderr before completing the protocol exchange",
                    ));
                }
                self.append_diagnostics(&diagnostic[..size]);
                Err(ResidentWorkerError::new(
                    error_code,
                    "worker wrote diagnostics during the protocol exchange",
                ))
            }
            result = read_frame(&mut self.stdout) => {
                result.map_err(|error| ResidentWorkerError::new(error_code, error))
            }
        }
    }

    async fn capture_remaining_diagnostics(&mut self) -> std::io::Result<()> {
        let mut buffer = [0_u8; 8192];
        loop {
            let size = self.stderr.read(&mut buffer).await?;
            if size == 0 {
                return Ok(());
            }
            self.append_diagnostics(&buffer[..size]);
        }
    }

    fn append_diagnostics(&mut self, bytes: &[u8]) {
        let remaining = MAX_DIAGNOSTIC_BYTES.saturating_sub(self.diagnostics.len());
        self.diagnostics
            .extend_from_slice(&bytes[..bytes.len().min(remaining)]);
    }

    async fn terminate_preserving(&mut self, original: ResidentWorkerError) -> ResidentWorkerError {
        match self.terminate().await {
            Ok(()) => original,
            Err(cleanup) => ResidentWorkerError::new(
                "WORKER_TERMINATION_FAILURE",
                format!("original failure: {original}; containment failure: {cleanup}"),
            ),
        }
    }

    async fn terminate(&mut self) -> Result<(), ResidentWorkerError> {
        #[cfg(unix)]
        {
            if let Err(error) = self.child.signal(nix::sys::signal::Signal::SIGTERM as i32) {
                if error.raw_os_error() != Some(nix::errno::Errno::ESRCH as i32) {
                    return Err(ResidentWorkerError::new(
                        "WORKER_TERMINATION_FAILURE",
                        error,
                    ));
                }
            }
            let graceful = timeout(SHUTDOWN_TIMEOUT, async {
                self.child.inner_mut().wait().await?;
                wait_for_group(self.pgid).await
            })
            .await;
            if matches!(graceful, Ok(Ok(()))) {
                return Ok(());
            }
        }

        let kill = self.child.start_kill();
        #[cfg(unix)]
        let kill = kill.or_else(|error| {
            if error.raw_os_error() == Some(nix::errno::Errno::ESRCH as i32) {
                Ok(())
            } else {
                Err(error)
            }
        });
        kill.map_err(|error| ResidentWorkerError::new("WORKER_TERMINATION_FAILURE", error))?;

        #[cfg(unix)]
        {
            timeout(REAP_TIMEOUT, async {
                self.child.inner_mut().wait().await?;
                wait_for_group(self.pgid).await
            })
            .await
            .map_err(|error| {
                ResidentWorkerError::new(
                    "WORKER_TERMINATION_FAILURE",
                    format!("worker process group was not reaped before deadline: {error}"),
                )
            })?
            .map_err(|error| ResidentWorkerError::new("WORKER_TERMINATION_FAILURE", error))?;
        }
        #[cfg(windows)]
        {
            // Wrapper wait is safe only after TerminateJobObject succeeded.
            timeout(REAP_TIMEOUT, self.child.wait())
                .await
                .map_err(|error| {
                    ResidentWorkerError::new(
                        "WORKER_TERMINATION_FAILURE",
                        format!("worker job was not reaped before deadline: {error}"),
                    )
                })?
                .map_err(|error| ResidentWorkerError::new("WORKER_TERMINATION_FAILURE", error))?;
        }
        Ok(())
    }
}

/// Per-request owned temporary directory. Mirrors the CLI supervisor: the
/// controller creates the directory before dispatch and settle removes it on
/// every settle path. The worker only borrows the directory for the request.
struct RequestTemp {
    dir: Option<tempfile::TempDir>,
}

impl RequestTemp {
    fn new(parent: &Option<PathBuf>) -> Result<Self, ResidentWorkerError> {
        let mut builder = tempfile::Builder::new();
        builder.prefix("ytdlp-worker-request-");
        let dir = match parent {
            Some(parent) => builder.tempdir_in(parent),
            None => builder.tempdir(),
        }
        .map_err(|error| ResidentWorkerError::new("TEMP_FAILURE", error))?;
        Ok(Self { dir: Some(dir) })
    }

    fn path(&self) -> &Path {
        match &self.dir {
            Some(dir) => dir.path(),
            None => Path::new(""),
        }
    }

    /// Remove the owned directory. A failed removal preserves the directory and
    /// surfaces the error (with its path), exactly like the CLI supervisor's
    /// close_temp.
    fn settle(self) -> Option<ResidentWorkerError> {
        let dir = self.dir?;
        let path = dir.path().to_path_buf();
        dir.close()
            .map_err(|error| {
                ResidentWorkerError::new(
                    "TEMP_CLEANUP_FAILURE",
                    format!("{}: {error}", path.display()),
                )
            })
            .err()
    }
}

fn cancellation_frame(request_id: &str) -> Result<Frame, crate::worker_protocol::ProtocolError> {
    Frame::new(MessageType::Cancel, request_id, json!({}))
}

#[cfg(unix)]
async fn wait_for_group(pgid: i32) -> std::io::Result<()> {
    use nix::{errno::Errno, sys::signal::killpg, unistd::Pid};

    loop {
        match killpg(Pid::from_raw(pgid), None) {
            Err(Errno::ESRCH) => return Ok(()),
            Err(error) => return Err(error.into()),
            Ok(()) => tokio::time::sleep(Duration::from_millis(20)).await,
        }
    }
}

fn join_python_path(
    entries: &[PathBuf],
) -> Result<Option<std::ffi::OsString>, ResidentWorkerError> {
    if entries.is_empty() {
        return Ok(None);
    }
    std::env::join_paths(entries)
        .map(Some)
        .map_err(|error| ResidentWorkerError::new("WORKER_CONFIG_FAILURE", error))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::cancellation_frame;
    use crate::worker_protocol::MessageType;
    use serde_json::json;

    #[test]
    fn cancellation_uses_envelope_request_id_and_empty_payload() {
        let frame = cancellation_frame("request-42").unwrap();

        assert_eq!(frame.message_type, MessageType::Cancel);
        assert_eq!(frame.request_id, "request-42");
        assert_eq!(frame.payload, json!({}));
    }
}
