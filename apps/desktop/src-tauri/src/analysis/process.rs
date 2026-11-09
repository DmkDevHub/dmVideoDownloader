use std::{
    io::Read,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    os::windows::process::CommandExt,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

fn job() -> Result<OwnedHandle, RunError> {
    unsafe {
        let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if handle.is_null() {
            return Err(RunError::Start);
        }
        let handle = OwnedHandle::from_raw_handle(handle);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if SetInformationJobObject(
            handle.as_raw_handle(),
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        ) == 0
        {
            return Err(RunError::Start);
        }
        Ok(handle)
    }
}

pub struct Output {
    pub success: bool,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunError {
    Start,
    Io,
    Timeout,
    OutputLimit,
}

fn collect(
    reader: impl Read + Send + 'static,
    limit: usize,
) -> mpsc::Receiver<Result<Vec<u8>, RunError>> {
    let (send, receive) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let result = (|| {
            let mut bytes = Vec::new();
            reader
                .take(limit as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| RunError::Io)?;
            if bytes.len() > limit {
                return Err(RunError::OutputLimit);
            }
            Ok(bytes)
        })();
        let _ = send.send(result);
    });
    receive
}

pub fn run(
    command: &mut Command,
    timeout: Duration,
    stdout_limit: usize,
    stderr_limit: usize,
) -> Result<Output, RunError> {
    let scratch = tempfile::tempdir().map_err(|_| RunError::Io)?;
    let job = job()?;
    command
        .current_dir(scratch.path())
        .env_clear()
        .env("TEMP", scratch.path())
        .env("TMP", scratch.path());
    if let Some(root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", root);
    }
    let mut child = command
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| RunError::Start)?;
    if unsafe { AssignProcessToJobObject(job.as_raw_handle(), child.as_raw_handle()) } == 0 {
        // Some version commands finish before assignment.
        if !matches!(child.try_wait(), Ok(Some(_))) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(RunError::Start);
        }
    }
    let stdout = collect(child.stdout.take().expect("piped stdout"), stdout_limit);
    let stderr = collect(child.stderr.take().expect("piped stderr"), stderr_limit);
    let deadline = Instant::now() + timeout;
    let result = (|| {
        let mut out = None;
        let mut err = None;
        let mut status = None;
        loop {
            if out.is_none() {
                out = poll(&stdout)?;
            }
            if err.is_none() {
                err = poll(&stderr)?;
            }
            if status.is_none() {
                status = child.try_wait().map_err(|_| RunError::Io)?;
            }
            if out.is_some() && err.is_some() {
                if let Some(exit) = status {
                    return Ok(Output {
                        success: exit.success(),
                        stdout: out.take().unwrap(),
                        stderr: err.take().unwrap(),
                    });
                }
            }
            if Instant::now() >= deadline {
                return Err(RunError::Timeout);
            }
            thread::sleep(Duration::from_millis(15));
        }
    })();
    drop(job);
    if result.is_err() {
        let _ = child.wait();
    }
    result
}

fn poll(receiver: &mpsc::Receiver<Result<Vec<u8>, RunError>>) -> Result<Option<Vec<u8>>, RunError> {
    match receiver.try_recv() {
        Ok(result) => result.map(Some),
        Err(mpsc::TryRecvError::Empty) => Ok(None),
        Err(mpsc::TryRecvError::Disconnected) => Err(RunError::Io),
    }
}
