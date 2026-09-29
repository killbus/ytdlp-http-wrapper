//! A handler may disappear; ownership of its process and temporary files must not.
use std::{io, process::ExitStatus, process::Stdio, sync::Arc, time::Duration};

#[cfg(windows)]
use process_wrap::tokio::JobObject;
#[cfg(unix)]
use process_wrap::tokio::ProcessGroup;
use process_wrap::tokio::{ChildWrapper, CommandWrap, KillOnDrop};
use tempfile::TempDir;
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    sync::{oneshot, OwnedSemaphorePermit},
    time::{timeout, Instant},
};
use tracing::{error, info, warn};

use crate::{models::RunResponse, routes::AppState};

const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;
#[cfg(unix)]
const TERMINATION_GRACE: Duration = Duration::from_secs(3);
const REAP_TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) struct Failure {
    pub code: &'static str,
    pub message: String,
}

impl Failure {
    fn new(code: &'static str, error: impl std::fmt::Display) -> Self {
        Self {
            code,
            message: error.to_string(),
        }
    }
}

pub(crate) type Reply = Result<RunResponse, Failure>;

// Drop is a last resort, not successful cleanup. Preserve files and close admission
// if a panic/runtime abort interrupts the supervised path.
struct Resources {
    child: Option<Box<dyn ChildWrapper>>,
    temp: Option<TempDir>,
    state: Arc<AppState>,
    _permit: OwnedSemaphorePermit,
    finished: bool,
    kill_sent: bool,
}

impl Drop for Resources {
    fn drop(&mut self) {
        if !self.finished {
            self.state.begin_shutdown();
            if !self.kill_sent {
                if let Some(child) = self.child.as_mut() {
                    let _ = child.start_kill();
                }
            }
            if let Some(temp) = self.temp.take() {
                error!(path = %temp.keep().display(), "Incomplete process cleanup; temporary directory preserved and admission closed");
            }
        }
    }
}

struct Drain<R> {
    reader: Option<R>,
    bytes: Vec<u8>,
    truncated: bool,
}

impl<R: AsyncRead + Unpin> Drain<R> {
    fn new(reader: Option<R>) -> Self {
        Self {
            reader,
            bytes: Vec::new(),
            truncated: false,
        }
    }

    // Cancellation-safe reads keep the captured prefix across termination stages.
    async fn finish(&mut self) -> io::Result<()> {
        if let Some(reader) = self.reader.as_mut() {
            let mut chunk = [0; 8192];
            loop {
                let size = reader.read(&mut chunk).await?;
                if size == 0 {
                    break;
                }
                let keep = size.min(MAX_OUTPUT_BYTES - self.bytes.len());
                self.bytes.extend_from_slice(&chunk[..keep]);
                self.truncated |= keep < size;
            }
        }
        self.reader = None;
        Ok(())
    }

    fn output(&self) -> String {
        String::from_utf8_lossy(&self.bytes).into_owned()
    }
}

async fn collect(
    child: &mut dyn ChildWrapper,
    stdout: &mut Drain<tokio::process::ChildStdout>,
    stderr: &mut Drain<tokio::process::ChildStderr>,
) -> io::Result<ExitStatus> {
    // Wrapper wait() can start an uncancellable blocking group/job reaper.
    // Execution deadlines only wait on Tokio's direct child and the two pipes.
    let (status, out, err) =
        tokio::join!(child.inner_mut().wait(), stdout.finish(), stderr.finish());
    out?;
    err?;
    status
}

#[cfg(unix)]
async fn wait_for_group(pgid: i32) -> io::Result<()> {
    use nix::{errno::Errno, sys::signal::killpg, unistd::Pid};
    loop {
        match killpg(Pid::from_raw(pgid), None) {
            Err(Errno::ESRCH) => return Ok(()),
            Err(e) => return Err(e.into()),
            Ok(()) => tokio::time::sleep(Duration::from_millis(20)).await,
        }
    }
}

pub(crate) async fn supervise(
    args: Vec<String>,
    duration: Duration,
    state: Arc<AppState>,
    permit: OwnedSemaphorePermit,
    mut reply: oneshot::Sender<Reply>,
) {
    let started = Instant::now();
    let mut resources = Resources {
        child: None,
        temp: None,
        state: state.clone(),
        _permit: permit,
        finished: false,
        kill_sent: false,
    };
    let result = run(&args, duration, &mut resources, &mut reply).await;
    info!(
        duration_ms = started.elapsed().as_millis() as u64,
        success = result.is_ok(),
        "yt-dlp supervision finished (including cleanup)"
    );
    if let Err(failure) = &result {
        error!(code = failure.code, error = %failure.message, "yt-dlp supervision failed");
    }
    // Release the permit before publishing the response, but only after cleanup.
    drop(resources);
    let _ = reply.send(result);
}

async fn run(
    args: &[String],
    duration: Duration,
    resources: &mut Resources,
    reply: &mut oneshot::Sender<Reply>,
) -> Reply {
    if resources.state.shutdown.is_cancelled() || reply.is_closed() {
        resources.finished = true;
        return Err(Failure::new(
            "SHUTTING_DOWN",
            "Request cancelled before process start",
        ));
    }
    let mut builder = tempfile::Builder::new();
    builder.prefix("ytdlp-request-");
    let temp = match &resources.state.temp_dir {
        Some(parent) => builder.tempdir_in(parent),
        None => builder.tempdir(),
    }
    .map_err(|e| {
        resources.finished = true;
        Failure::new("TEMP_FAILURE", e)
    })?;
    let temp_path = temp.path().canonicalize();
    resources.temp = Some(temp);
    let temp_path = match temp_path {
        Ok(path) => path,
        Err(e) => {
            close_temp(resources)?;
            return Err(Failure::new("TEMP_FAILURE", e));
        }
    };
    let mut command = CommandWrap::with_new(&resources.state.binary_path, |cmd| {
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for key in ["TMPDIR", "TMP", "TEMP"] {
            cmd.env(key, &temp_path);
        }
    });
    command.wrap(KillOnDrop);
    #[cfg(unix)]
    command.wrap(ProcessGroup::leader());
    #[cfg(windows)]
    command.wrap(JobObject);

    match command.spawn() {
        Ok(child) => resources.child = Some(child),
        Err(e) => {
            close_temp(resources)?;
            return Err(Failure::new("SPAWN_FAILURE", e));
        }
    }
    let child = resources
        .child
        .as_mut()
        .ok_or_else(|| Failure::new("SPAWN_FAILURE", "Missing child"))?;
    #[cfg(unix)]
    let pgid = child
        .id()
        .and_then(|id| i32::try_from(id).ok())
        .ok_or_else(|| Failure::new("SPAWN_FAILURE", "Missing process group ID"))?;
    let mut stdout = Drain::new(child.stdout().take());
    let mut stderr = Drain::new(child.stderr().take());
    let (completed, reason) = tokio::select! {
        biased;
        _ = resources.state.shutdown.cancelled() => (None, "shutdown"),
        _ = reply.closed() => (None, "handler_cancelled"),
        result = timeout(duration, collect(child.as_mut(), &mut stdout, &mut stderr)) => {
            match result {
                Ok(result) => (Some(result), "completed"),
                Err(_) => (None, "timeout"),
            }
        },
    };
    let interrupted = completed.is_none();
    if interrupted {
        warn!(reason, "Terminating yt-dlp process tree");
        #[cfg(unix)]
        {
            if let Err(e) = child.signal(nix::sys::signal::Signal::SIGTERM as i32) {
                if e.raw_os_error() != Some(nix::errno::Errno::ESRCH as i32) {
                    warn!(error = %e, "SIGTERM failed; escalating");
                }
            }
            let _ = timeout(TERMINATION_GRACE, async {
                let status = collect(child.as_mut(), &mut stdout, &mut stderr).await?;
                wait_for_group(pgid).await?;
                Ok::<_, io::Error>(status)
            })
            .await;
        }
    }
    // Even a successful leader may leave descendants with closed stdout/stderr.
    let kill = child.start_kill();
    #[cfg(unix)]
    let kill = kill.or_else(|e| {
        if e.raw_os_error() == Some(nix::errno::Errno::ESRCH as i32) {
            Ok(())
        } else {
            Err(e)
        }
    });
    kill.map_err(|e| Failure::new("CLEANUP_FAILURE", e))?;
    resources.kill_sent = true;
    let cleanup = timeout(REAP_TIMEOUT, async {
        let collected = collect(child.as_mut(), &mut stdout, &mut stderr).await;
        #[cfg(unix)]
        wait_for_group(pgid).await?;
        #[cfg(windows)]
        child.wait().await?; // Only after successful TerminateJobObject.
        collected
    })
    .await;
    let status = match cleanup {
        Ok(Ok(status)) => status,
        Ok(Err(e)) => return Err(Failure::new("CLEANUP_FAILURE", e)),
        Err(e) => {
            #[cfg(windows)]
            if let Some(child) = resources.child.take() {
                // process-wrap may be polling a raw completion-port handle on a
                // blocking thread. Keep it valid if the OS cannot finish the kill.
                std::mem::forget(child);
            }
            return Err(Failure::new("CLEANUP_FAILURE", e));
        }
    };
    if stdout.truncated || stderr.truncated {
        warn!(
            stdout_truncated = stdout.truncated,
            stderr_truncated = stderr.truncated,
            limit = MAX_OUTPUT_BYTES,
            "Process output truncated"
        );
    }
    resources.child.take();
    close_temp(resources)?;
    if let Some(Err(e)) = completed {
        return Err(Failure::new("COLLECT_FAILURE", e));
    }
    Ok(RunResponse {
        exit_code: if interrupted {
            -1
        } else {
            status.code().unwrap_or(-1)
        },
        stdout: stdout.output(),
        stderr: stderr.output(),
    })
}

fn close_temp(resources: &mut Resources) -> Result<(), Failure> {
    if let Some(temp) = resources.temp.take() {
        let path = temp.path().to_path_buf();
        temp.close().map_err(|e| {
            Failure::new("TEMP_CLEANUP_FAILURE", format!("{}: {e}", path.display()))
        })?;
    }
    resources.finished = true;
    Ok(())
}
