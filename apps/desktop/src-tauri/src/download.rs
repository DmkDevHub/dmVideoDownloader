use crate::analysis::{extract, failure, process, proxy};
use crate::{
    bridge::Bridge,
    folders,
    settings::{Settings, Store},
};
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tauri::{ipc::Channel, State};
use windows_sys::Win32::Storage::FileSystem::MoveFileExW;

const TRANSFER_TIMEOUT: Duration = Duration::from_secs(600);
static ACTIVE: AtomicBool = AtomicBool::new(false);

struct Active;
impl Drop for Active {
    fn drop(&mut self) {
        ACTIVE.store(false, Ordering::Release);
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Request {
    Direct { address: String },
    Candidate { id: String },
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    downloaded: u64,
    total: Option<u64>,
    bytes_per_second: Option<f64>,
}

fn progress(line: &[u8]) -> Option<Progress> {
    let text = std::str::from_utf8(line).ok()?.trim();
    let mut fields = text.strip_prefix("DM_PROGRESS ")?.split_whitespace();
    let downloaded = fields.next()?.parse().ok()?;
    let exact = fields.next()?.parse().ok();
    let estimate = fields.next()?.parse().ok();
    let bytes_per_second = fields
        .next()?
        .parse::<f64>()
        .ok()
        .filter(|speed| speed.is_finite() && *speed >= 0.0);
    Some(Progress {
        downloaded,
        total: exact
            .or(estimate)
            .filter(|total| *total >= downloaded && *total > 0),
        bytes_per_second,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Saved {
    filename: String,
    height: Option<u32>,
}

#[tauri::command]
pub async fn download_video(
    request: Request,
    updates: Channel<Progress>,
    store: State<'_, Mutex<Store>>,
    bridge: State<'_, Arc<Bridge>>,
) -> Result<Saved, String> {
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
        let mut store = store.lock().map_err(|_| "SETTINGS_BUSY")?;
        let snapshot = store.load()?;
        if !snapshot.can_save {
            return Err(snapshot.notice.unwrap_or_else(|| "INVALID_SETTINGS".into()));
        }
        (store.root().to_path_buf(), snapshot.settings)
    };
    ACTIVE
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .map_err(|_| "DOWNLOAD_BUSY")?;
    let active = Active;
    tauri::async_runtime::spawn_blocking(move || {
        let _active = active;
        download(&root, &settings, &page, media.as_deref(), move |update| {
            let _ = updates.send(update);
        })
    })
    .await
    .map_err(|_| "DOWNLOAD_FAILED")?
    .map_err(str::to_owned)
}

fn download(
    root: &Path,
    settings: &Settings,
    page: &str,
    media: Option<&str>,
    update: impl FnMut(Progress) + Send + 'static,
) -> Result<Saved, &'static str> {
    let analyzer = extract::Analyzer::open(root)?;
    let report = extract::candidate(page, media, |address| analyzer.analyze(address))?;
    let format = report
        .formats
        .iter()
        .find(|format| {
            matches!(
                format.tracks,
                extract::Tracks::AudioVideo | extract::Tracks::VideoOnly | extract::Tracks::Unknown
            ) && matches!(format.container.as_deref(), Some("mp4" | "webm"))
                && format
                    .media
                    .as_ref()
                    .is_some_and(|media| matches!(media.protocol.as_str(), "http" | "https"))
        })
        .ok_or("DOWNLOAD_FORMAT_UNSUPPORTED")?;
    let ext = format
        .container
        .as_deref()
        .ok_or("DOWNLOAD_FORMAT_UNSUPPORTED")?;
    let directory = folders::validate_directory(&settings.download_directory)?;
    let staging = tempfile::Builder::new()
        .prefix(".dm-video-")
        .tempdir_in(&directory)
        .map_err(|_| "DIRECTORY_NOT_WRITABLE")?;
    let path = staging.path().join(format!("source.{ext}"));
    transfer(&analyzer, format, &path, update)?;
    let filename = format!("video.{ext}");
    move_without_replacing(&path, &directory.join(&filename)).map_err(|error| {
        if error.kind() == io::ErrorKind::AlreadyExists {
            "DOWNLOAD_EXISTS"
        } else {
            "DOWNLOAD_SAVE_FAILED"
        }
    })?;
    Ok(Saved {
        filename,
        height: format.height,
    })
}

fn transfer(
    analyzer: &extract::Analyzer,
    format: &extract::Format,
    path: &Path,
    mut update: impl FnMut(Progress) + Send + 'static,
) -> Result<(), &'static str> {
    let media = format.media.as_ref().ok_or("DOWNLOAD_FORMAT_UNSUPPORTED")?;
    let proxy = proxy::Proxy::download(TRANSFER_TIMEOUT).map_err(|_| "NETWORK_ERROR")?;
    // Construct a new info object. Never pass through extractor metadata, headers, or file paths.
    let input = serde_json::to_vec(&serde_json::json!({
        "id": "public-video",
        "title": "Public video",
        "url": media.url.as_str(),
        "protocol": media.protocol,
        "ext": format.container,
        "vcodec": format.video_codec,
        "acodec": format.audio_codec,
    }))
    .map_err(|_| "DOWNLOAD_FAILED")?;
    let mut command = analyzer.command(&proxy);
    let template = path.to_str().ok_or("INVALID_DIRECTORY")?.replace('%', "%%");
    command.args([
        "--load-info-json", "-", "--downloader", "native", "--fixup", "never",
        "--no-mtime", "--newline", "--progress", "--progress-delta", "0.25",
        "--progress-template", "download:DM_PROGRESS %(progress.downloaded_bytes)s %(progress.total_bytes)s %(progress.total_bytes_estimate)s %(progress.speed)s",
        "--output", &template,
    ]);
    let output = process::run_with(
        &mut command,
        TRANSFER_TIMEOUT,
        (512 * 1024, 64 * 1024),
        Some(input),
        move |line| {
            if let Some(value) = progress(line) {
                update(value);
            }
        },
    );
    if proxy.blocked() {
        return Err("URL_NOT_ALLOWED");
    }
    let output = output.map_err(|error| match error {
        process::RunError::Timeout => "DOWNLOAD_TIMEOUT",
        _ => "DOWNLOAD_FAILED",
    })?;
    if !output.success {
        let reason = failure::classify(&output.stderr);
        return Err(if reason == "ANALYSIS_FAILED" {
            "DOWNLOAD_FAILED"
        } else {
            reason
        });
    }
    folders::check_path(path)?;
    if !fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0) {
        return Err("DOWNLOAD_FAILED");
    }
    Ok(())
}

fn move_without_replacing(source: &Path, destination: &Path) -> io::Result<()> {
    let checked = |path: &Path| folders::check_path(path).map_err(io::Error::other);
    checked(source)?;
    checked(
        destination
            .parent()
            .ok_or_else(|| io::Error::other("invalid destination"))?,
    )?;
    let wide = |path: &Path| {
        path.as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>()
    };
    if unsafe { MoveFileExW(wide(source).as_ptr(), wide(destination).as_ptr(), 0) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_numeric_progress_without_disclosing_raw_output() {
        let value = progress(b"DM_PROGRESS 128 256 NA 2000.0\n").unwrap();
        assert_eq!(value.downloaded, 128);
        assert_eq!(value.total, Some(256));
        assert_eq!(value.bytes_per_second, Some(2000.0));
        assert!(progress(b"ERROR: https://example.org/?token=value").is_none());
        let unknown = progress(b"DM_PROGRESS 300 NA 256 NaN\n").unwrap();
        assert_eq!(unknown.total, None);
        assert_eq!(unknown.bytes_per_second, None);
    }

    #[test]
    #[ignore = "requires verified developer binaries and public network access"]
    fn real_public_mp4_download_and_decode() {
        let root = PathBuf::from(std::env::var_os("DM_R4_TOOL_ROOT").unwrap());
        let output = tempfile::tempdir().unwrap();
        let settings = Settings::defaults(output.path().to_str().unwrap().into());
        let (send, receive) = std::sync::mpsc::channel();
        let result = download(
            &root,
            &settings,
            "https://interactive-examples.mdn.mozilla.net/media/cc0-videos/flower.mp4",
            None,
            move |value| {
                send.send(value).unwrap();
            },
        )
        .unwrap();
        let file = output.path().join(result.filename);
        assert_eq!(fs::metadata(&file).unwrap().len(), 1_128_375);
        assert!(receive
            .try_iter()
            .any(|value| value.downloaded > 0 && value.total.is_some()));
        assert_eq!(fs::read_dir(output.path()).unwrap().count(), 1);
        decode(&root, &file);
        println!("MP4_DOWNLOAD_AND_DECODE_PASS");
    }

    fn decode(root: &Path, file: &Path) {
        use std::process::Command;
        let output = process::run(
            Command::new(root.join("tools/ffmpeg.exe"))
                .args(["-nostdin", "-v", "error", "-xerror", "-i"])
                .arg(file)
                .args(["-map", "0:v:0", "-f", "null", "-"]),
            Duration::from_secs(30),
            4096,
            4096,
        )
        .unwrap();
        assert!(output.success, "FFmpeg decode failed");
    }
}
