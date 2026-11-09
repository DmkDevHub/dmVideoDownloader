use super::{download, proxy, Progress, Saved, Update};
use crate::{
    bridge::Bridge,
    settings::{Settings, Store},
};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use tauri::State;

const MAX_JOBS: usize = 64;

pub(super) struct Work {
    pub root: PathBuf,
    pub settings: Settings,
    pub page: String,
    pub media: Option<String>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Signal {
    #[default]
    Run,
    Pause,
    Cancel,
    Committed,
}

#[derive(Default)]
pub(super) struct Control {
    signal: Mutex<Signal>,
    wake: Condvar,
}

impl Control {
    pub fn interrupted(&self) -> bool {
        matches!(*self.signal.lock().unwrap(), Signal::Pause | Signal::Cancel)
    }
    pub fn check(&self) -> Result<(), &'static str> {
        if self.interrupted() {
            Err("DOWNLOAD_INTERRUPTED")
        } else {
            Ok(())
        }
    }
    fn interrupt(&self, signal: Signal) -> Result<(), &'static str> {
        let mut current = self.signal.lock().unwrap();
        if *current != Signal::Run && !(*current == Signal::Pause && signal == Signal::Cancel) {
            return Err("DOWNLOAD_ACTION_UNAVAILABLE");
        }
        *current = signal;
        self.wake.notify_all();
        Ok(())
    }
    fn wait(&self, delay: Duration) {
        let signal = self.signal.lock().unwrap();
        drop(
            self.wake
                .wait_timeout_while(signal, delay, |signal| *signal == Signal::Run)
                .unwrap(),
        );
    }
    pub fn commit<T>(
        &self,
        publish: impl FnOnce() -> Result<T, &'static str>,
    ) -> Result<T, &'static str> {
        let mut signal = self.signal.lock().unwrap();
        if *signal != Signal::Run {
            return Err("DOWNLOAD_INTERRUPTED");
        }
        let result = publish()?;
        *signal = Signal::Committed;
        Ok(result)
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Request {
    Direct { address: String },
    Candidate { id: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
enum Phase {
    Queued,
    Analyzing,
    Downloading,
    Merging,
    Retrying,
    Pausing,
    Cancelling,
    Paused,
    Completed,
    Failed,
    Cancelled,
}
impl Phase {
    fn terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobView {
    id: u32,
    title: String,
    phase: Phase,
    height: Option<u32>,
    progress: Option<Progress>,
    filename: Option<String>,
    error: Option<&'static str>,
    attempt: u8,
    can_retry: bool,
}

struct Job {
    view: JobView,
    work: Option<Arc<Work>>,
    control: Option<Arc<Control>>,
}

#[derive(Default)]
struct Jobs {
    next: u32,
    entries: Vec<Job>,
    closing: bool,
}

#[derive(Default)]
pub struct Queue {
    jobs: Mutex<Jobs>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    Pause,
    Resume,
    Cancel,
    Retry,
}

impl Queue {
    fn enqueue(&self, work: Work) -> Result<u32, &'static str> {
        let mut jobs = self.jobs.lock().unwrap();
        if jobs.closing {
            return Err("DOWNLOAD_ACTION_UNAVAILABLE");
        }
        if jobs.entries.len() >= MAX_JOBS {
            let index = jobs
                .entries
                .iter()
                .position(|job| job.view.phase.terminal())
                .ok_or("DOWNLOAD_QUEUE_FULL")?;
            jobs.entries.remove(index);
        }
        jobs.next = jobs.next.checked_add(1).ok_or("DOWNLOAD_QUEUE_FULL")?;
        let id = jobs.next;
        jobs.entries.push(Job {
            view: JobView {
                id,
                title: "공개 영상".into(),
                phase: Phase::Queued,
                height: None,
                progress: None,
                filename: None,
                error: None,
                attempt: 1,
                can_retry: false,
            },
            work: Some(Arc::new(work)),
            control: None,
        });
        Ok(id)
    }

    fn action(&self, id: u32, action: Action) -> Result<(), &'static str> {
        let mut jobs = self.jobs.lock().unwrap();
        let job = jobs
            .entries
            .iter_mut()
            .find(|job| job.view.id == id)
            .ok_or("DOWNLOAD_JOB_MISSING")?;
        let phase = job.view.phase;
        match (action, phase, job.control.as_ref()) {
            (Action::Pause, Phase::Queued, None) => job.view.phase = Phase::Paused,
            (
                Action::Pause,
                Phase::Analyzing | Phase::Downloading | Phase::Retrying,
                Some(control),
            ) => {
                control.interrupt(Signal::Pause)?;
                job.view.phase = Phase::Pausing;
            }
            (Action::Cancel, Phase::Queued | Phase::Paused, None) => {
                job.view.phase = Phase::Cancelled;
                job.work = None;
            }
            (
                Action::Cancel,
                Phase::Analyzing
                | Phase::Downloading
                | Phase::Merging
                | Phase::Retrying
                | Phase::Pausing,
                Some(control),
            ) => {
                control.interrupt(Signal::Cancel)?;
                job.view.phase = Phase::Cancelling;
            }
            (Action::Resume, Phase::Paused, None) => job.view.phase = Phase::Queued,
            (Action::Retry, Phase::Failed, None) if job.view.can_retry => {
                job.view.phase = Phase::Queued
            }
            _ => return Err("DOWNLOAD_ACTION_UNAVAILABLE"),
        }
        job.view.error = None;
        job.view.can_retry = false;
        job.view.progress = None;
        job.view.attempt = 1;
        Ok(())
    }

    fn claim(&self) -> Option<(u32, Arc<Work>, Arc<Control>)> {
        let mut jobs = self.jobs.lock().unwrap();
        if jobs.closing || jobs.entries.iter().any(|job| job.control.is_some()) {
            return None;
        }
        let job = jobs
            .entries
            .iter_mut()
            .find(|job| job.view.phase == Phase::Queued)?;
        let work = Arc::clone(job.work.as_ref()?);
        let control = Arc::new(Control::default());
        job.control = Some(Arc::clone(&control));
        job.view.phase = Phase::Analyzing;
        Some((job.view.id, work, control))
    }

    fn update(&self, id: u32, apply: impl FnOnce(&mut JobView)) {
        let mut jobs = self.jobs.lock().unwrap();
        if let Some(job) = jobs.entries.iter_mut().find(|job| job.view.id == id) {
            if !matches!(job.view.phase, Phase::Pausing | Phase::Cancelling) {
                apply(&mut job.view);
            }
        }
    }

    fn schedule(self: &Arc<Self>) {
        if let Some((id, work, control)) = self.claim() {
            let queue = Arc::clone(self);
            tauri::async_runtime::spawn_blocking(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    queue.run(id, &work, &control)
                }))
                .unwrap_or(Err("DOWNLOAD_FAILED"));
                queue.finish(id, result);
                queue.schedule();
            });
        }
    }

    fn run(
        self: &Arc<Self>,
        id: u32,
        work: &Work,
        control: &Control,
    ) -> Result<Saved, &'static str> {
        for attempt in 0..=work.settings.retry_count {
            control.check()?;
            self.update(id, |job| {
                job.phase = Phase::Analyzing;
                job.progress = None;
                job.error = None;
                job.attempt = attempt + 1;
            });
            let queue = Arc::clone(self);
            let result = download(work, control, move |update| {
                queue.update(id, |job| match update {
                    Update::Downloading { title, height } => {
                        job.title = title;
                        job.height = height;
                        job.phase = Phase::Downloading;
                    }
                    Update::Progress(progress) => job.progress = Some(progress),
                    Update::Merging => {
                        job.phase = Phase::Merging;
                        job.progress = None;
                    }
                })
            });
            match result {
                Err(code)
                    if retryable(code)
                        && attempt < work.settings.retry_count
                        && !control.interrupted() =>
                {
                    self.update(id, |job| {
                        job.phase = Phase::Retrying;
                        job.error = Some(code);
                        job.progress = None;
                    });
                    control.wait(Duration::from_secs(1 << attempt));
                }
                _ => return result,
            }
        }
        Err("DOWNLOAD_FAILED")
    }

    fn finish(&self, id: u32, result: Result<Saved, &'static str>) {
        let mut jobs = self.jobs.lock().unwrap();
        let Some(job) = jobs.entries.iter_mut().find(|job| job.view.id == id) else {
            return;
        };
        job.control = None;
        job.view.progress = None;
        job.view.error = None;
        job.view.phase = match (job.view.phase, result) {
            (_, Ok(saved)) => {
                job.view.filename = Some(saved.filename);
                job.view.height = saved.height;
                job.work = None;
                Phase::Completed
            }
            (Phase::Pausing, Err(_)) => Phase::Paused,
            (Phase::Cancelling, Err(_)) => {
                job.work = None;
                Phase::Cancelled
            }
            (_, Err(error)) => {
                job.view.error = Some(error);
                job.view.can_retry = retryable(error);
                if !job.view.can_retry {
                    job.work = None;
                }
                Phase::Failed
            }
        };
    }

    pub fn shutdown(&self) -> (bool, bool) {
        let mut jobs = self.jobs.lock().unwrap();
        let first = !jobs.closing;
        jobs.closing = true;
        let mut running = false;
        for job in &mut jobs.entries {
            if let Some(control) = &job.control {
                running = true;
                if control.interrupt(Signal::Cancel).is_ok() {
                    job.view.phase = Phase::Cancelling;
                }
            } else if matches!(job.view.phase, Phase::Queued | Phase::Paused) {
                job.view.phase = Phase::Cancelled;
                job.work = None;
            }
        }
        (running, first)
    }

    pub fn busy(&self) -> bool {
        self.jobs
            .lock()
            .unwrap()
            .entries
            .iter()
            .any(|job| job.control.is_some())
    }
}

fn retryable(code: &str) -> bool {
    matches!(
        code,
        "NETWORK_ERROR" | "ANALYSIS_TIMEOUT" | "DOWNLOAD_TIMEOUT"
    )
}

#[tauri::command]
pub fn download_video(
    request: Request,
    store: State<'_, Mutex<Store>>,
    bridge: State<'_, Arc<Bridge>>,
    queue: State<'_, Arc<Queue>>,
) -> Result<u32, String> {
    let (page, media) = match request {
        Request::Direct { address } => (address, None),
        Request::Candidate { id } => {
            let candidate = bridge.candidate(&id)?;
            (candidate.page_url, candidate.media_url)
        }
    };
    if proxy::target(&page).is_none() {
        return Err("URL_NOT_ALLOWED".into());
    }
    let (root, settings) = {
        let store = store.lock().map_err(|_| "SETTINGS_BUSY")?;
        (store.root().to_path_buf(), store.current_settings()?)
    };
    let id = queue.enqueue(Work {
        root,
        settings,
        page,
        media,
    })?;
    queue.inner().schedule();
    Ok(id)
}

#[tauri::command]
pub fn download_jobs(queue: State<'_, Arc<Queue>>) -> Vec<JobView> {
    queue
        .jobs
        .lock()
        .unwrap()
        .entries
        .iter()
        .map(|job| job.view.clone())
        .collect()
}

#[tauri::command]
pub fn download_action(
    id: u32,
    action: Action,
    queue: State<'_, Arc<Queue>>,
) -> Result<(), String> {
    queue.action(id, action)?;
    queue.inner().schedule();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn work() -> Work {
        Work {
            root: PathBuf::new(),
            settings: Settings::defaults(String::new()),
            page: "https://example.org/video".into(),
            media: None,
        }
    }

    #[test]
    fn queue_releases_slots_and_supports_pause_cancel_and_transient_retry() {
        let queue = Queue::default();
        let first = queue.enqueue(work()).unwrap();
        let second = queue.enqueue(work()).unwrap();
        assert_eq!(queue.claim().unwrap().0, first);
        assert!(queue.claim().is_none());
        queue.action(first, Action::Pause).unwrap();
        queue.finish(first, Err("DOWNLOAD_INTERRUPTED"));
        assert_eq!(queue.claim().unwrap().0, second);
        queue.action(first, Action::Resume).unwrap();
        queue.action(second, Action::Cancel).unwrap();
        queue.finish(second, Err("DOWNLOAD_INTERRUPTED"));
        assert_eq!(queue.claim().unwrap().0, first);
        queue.finish(first, Err("NETWORK_ERROR"));
        queue.action(first, Action::Retry).unwrap();
        assert_eq!(queue.claim().unwrap().0, first);
        queue.finish(first, Err("AUTH_REQUIRED"));
        assert!(queue.action(first, Action::Retry).is_err());
        let jobs = queue.jobs.lock().unwrap();
        assert_eq!(jobs.entries[0].view.phase, Phase::Failed);
        assert_eq!(jobs.entries[1].view.phase, Phase::Cancelled);
        assert!(jobs.entries.iter().all(|job| job.work.is_none()));
    }

    #[test]
    fn cancel_and_atomic_publish_cannot_both_win() {
        let control = Control::default();
        control.interrupt(Signal::Cancel).unwrap();
        assert_eq!(
            control.commit(|| panic!("cancelled job cannot publish")),
            Err::<(), _>("DOWNLOAD_INTERRUPTED")
        );
        let control = Control::default();
        assert_eq!(control.commit(|| Ok("saved")), Ok("saved"));
        assert!(control.interrupt(Signal::Cancel).is_err());
    }

    #[test]
    #[ignore = "requires verified developer binaries and public network access"]
    fn real_paused_transfer_cleans_parts_and_resumes_as_a_new_attempt() {
        let root = PathBuf::from(std::env::var_os("DM_R4_TOOL_ROOT").unwrap());
        let output = tempfile::tempdir().unwrap();
        let work = Work {
            root,
            settings: Settings::defaults(output.path().to_str().unwrap().into()),
            page: "https://interactive-examples.mdn.mozilla.net/media/cc0-videos/flower.mp4".into(),
            media: None,
        };
        let control = Arc::new(Control::default());
        let pause = Arc::clone(&control);
        let result = download(&work, &control, move |event| {
            if matches!(event, Update::Progress(_)) {
                let _ = pause.interrupt(Signal::Pause);
            }
        });
        assert_eq!(result.err(), Some("DOWNLOAD_INTERRUPTED"));
        assert_eq!(std::fs::read_dir(output.path()).unwrap().count(), 0);
        let finished = download(&work, &Control::default(), |_| {}).unwrap();
        assert!(output.path().join(finished.filename).is_file());
        println!("PAUSE_CLEANUP_AND_RESTART_PASS");
    }

    #[test]
    #[ignore = "requires verified developer binaries and public network access"]
    fn real_transient_failure_retries_but_authentication_does_not() {
        let root = PathBuf::from(std::env::var_os("DM_R4_TOOL_ROOT").unwrap());
        for (status, code, attempt) in [(503, "NETWORK_ERROR", 2), (401, "AUTH_REQUIRED", 1)] {
            let queue = Arc::new(Queue::default());
            let mut work = work();
            work.root = root.clone();
            work.page = format!("https://httpbin.org/status/{status}");
            work.settings.retry_count = 1;
            let id = queue.enqueue(work).unwrap();
            let (_, work, control) = queue.claim().unwrap();
            let result = queue.run(id, &work, &control);
            assert_eq!(result.as_ref().err(), Some(&code));
            queue.finish(id, result);
            let jobs = queue.jobs.lock().unwrap();
            assert_eq!(jobs.entries[0].view.attempt, attempt);
            assert_eq!(jobs.entries[0].view.can_retry, attempt == 2);
        }
        println!("TRANSIENT_RETRY_AND_AUTH_STOP_PASS");
    }
}
