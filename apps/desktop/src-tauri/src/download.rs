use crate::analysis::{extract, failure, process, proxy, tools};
use crate::{
    folders,
    settings::{Filename, Format as Container, Quality},
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs, io,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    time::Duration,
};
use windows_sys::Win32::Storage::FileSystem::MoveFileExW;

pub mod queue;
use queue::{Control, Work};
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(600);

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

struct Saved {
    filename: String,
    height: Option<u32>,
}

enum Update {
    Downloading { title: String, height: Option<u32> },
    Progress(Progress),
    Merging,
}

fn download(
    work: &Work,
    control: &Control,
    update: impl Fn(Update) + Send + Sync + 'static,
) -> Result<Saved, &'static str> {
    let (root, settings, page, media) = (
        &work.root,
        &work.settings,
        work.page.as_str(),
        work.media.as_deref(),
    );
    control.check()?;
    let analyzer = extract::Analyzer::open(root)?;
    let report = extract::candidate(page, media, |address| {
        analyzer.analyze_interruptible(address, || control.interrupted())
    })?;
    control.check()?;
    let format = choose_video(
        report.formats.iter().filter(|format| {
            matches!(
                format.tracks,
                extract::Tracks::AudioVideo | extract::Tracks::VideoOnly | extract::Tracks::Unknown
            ) && matches!(format.container.as_deref(), Some("mp4" | "webm"))
                && direct_transfer(format)
                && (settings.format != Container::Mp4 || format.container.as_deref() == Some("mp4"))
        }),
        &settings.quality,
    )?;
    let ext = format
        .container
        .as_deref()
        .ok_or("DOWNLOAD_FORMAT_UNSUPPORTED")?;
    let audio = if format.tracks == extract::Tracks::VideoOnly {
        report
            .formats
            .iter()
            .filter(|candidate| {
                candidate.tracks == extract::Tracks::AudioOnly
                    && direct_transfer(candidate)
                    && matches!(candidate.container.as_deref(), Some("m4a" | "webm"))
                    && (settings.format != Container::Mp4
                        || candidate.container.as_deref() == Some("m4a"))
            })
            .max_by_key(|candidate| (same_family(ext, candidate), candidate.bitrate.unwrap_or(0)))
    } else {
        None
    };
    if settings.format == Container::Mp4
        && audio.is_none()
        && format.tracks == extract::Tracks::VideoOnly
        && report.formats.iter().any(|candidate| {
            candidate.tracks == extract::Tracks::AudioOnly && direct_transfer(candidate)
        })
    {
        return Err("CONTAINER_INCOMPATIBLE");
    }
    let output_ext = match settings.format {
        Container::Mp4 => "mp4",
        Container::Mkv => "mkv",
        Container::Auto if audio.is_some_and(|audio| !same_family(ext, audio)) => "mkv",
        Container::Auto => ext,
    };
    let ffmpeg = ffmpeg(root)?;
    let directory = folders::validate_directory(&settings.download_directory)?;
    let source = if report.source == extract::Source::Media {
        media.unwrap_or(page)
    } else {
        page
    };
    let stem = filename_stem(
        &report.title,
        source,
        &settings.filename,
        &directory,
        output_ext,
    )?;
    let staging = tempfile::Builder::new()
        .prefix(".dm-video-")
        .tempdir_in(&directory)
        .map_err(|_| "DIRECTORY_NOT_WRITABLE")?;
    let video = staging.path().join(format!("source.{ext}"));
    let update = Arc::new(update);
    update(Update::Downloading {
        title: report.title,
        height: format.height,
    });
    let video_update = Arc::clone(&update);
    transfer(&analyzer, format, &video, control, move |value| {
        video_update(Update::Progress(value))
    })?;
    let audio_file = audio
        .map(|audio| -> Result<PathBuf, &'static str> {
            let ext = audio
                .container
                .as_deref()
                .ok_or("DOWNLOAD_FORMAT_UNSUPPORTED")?;
            let file = staging.path().join(format!("audio.{ext}"));
            let audio_update = Arc::clone(&update);
            transfer(&analyzer, audio, &file, control, move |value| {
                audio_update(Update::Progress(value))
            })?;
            Ok(file)
        })
        .transpose()?;
    update(Update::Merging);
    let path = staging.path().join(format!("finished.{output_ext}"));
    remux(&ffmpeg, &video, audio_file.as_deref(), &path, control)?;
    let filename = control.commit(|| publish(&path, &directory, &stem, output_ext))?;
    Ok(Saved {
        filename,
        height: format.height,
    })
}

fn filename_stem(
    title: &str,
    address: &str,
    rule: &Filename,
    directory: &Path,
    ext: &str,
) -> Result<String, &'static str> {
    let (prefix, suffix) = match rule {
        Filename::Title => (String::new(), String::new()),
        Filename::TitleId => {
            let mut url = proxy::target(address).ok_or("URL_NOT_ALLOWED")?;
            url.set_query(None);
            let hash = Sha256::digest(url.as_str().as_bytes());
            (
                String::new(),
                format!(
                    "-{:02x}{:02x}{:02x}{:02x}",
                    hash[0], hash[1], hash[2], hash[3]
                ),
            )
        }
        Filename::DateTitle => {
            use windows_sys::Win32::{
                Foundation::SYSTEMTIME, System::SystemInformation::GetLocalTime,
            };
            let mut date = SYSTEMTIME::default();
            unsafe { GetLocalTime(&mut date) };
            (
                format!("{:04}-{:02}-{:02}-", date.wYear, date.wMonth, date.wDay),
                String::new(),
            )
        }
    };
    // Leave room for the extension and a concurrent download's numbered suffix.
    let max = 240usize
        .saturating_sub(directory.as_os_str().len() + ext.len() + prefix.len() + suffix.len() + 10)
        .min(120);
    if max < "video_".len() {
        return Err("INVALID_DIRECTORY");
    }
    let mut name = String::new();
    for ch in title.split(['?', '#']).next().unwrap_or_default().chars() {
        let ch = if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '*')
        {
            '_'
        } else {
            ch
        };
        if name.len() + ch.len_utf8() >= max {
            break;
        }
        name.push(ch);
    }
    let name = name.trim_matches(['.', ' ']);
    let name = if name.is_empty() { "video" } else { name };
    let guard = if folders::reserved_name(name) {
        "_"
    } else {
        ""
    };
    Ok(format!("{prefix}{guard}{name}{suffix}"))
}

fn publish(source: &Path, directory: &Path, stem: &str, ext: &str) -> Result<String, &'static str> {
    for number in 0..=999 {
        let filename = if number == 0 {
            format!("{stem}.{ext}")
        } else {
            format!("{stem} ({number}).{ext}")
        };
        match move_without_replacing(source, &directory.join(&filename)) {
            Ok(()) => return Ok(filename),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err("DOWNLOAD_SAVE_FAILED"),
        }
    }
    Err("DOWNLOAD_EXISTS")
}

fn direct_transfer(format: &extract::Format) -> bool {
    format
        .media
        .as_ref()
        .is_some_and(|media| matches!(media.protocol.as_str(), "http" | "https"))
}

fn same_family(video_ext: &str, audio: &extract::Format) -> bool {
    matches!(
        (video_ext, audio.container.as_deref()),
        ("mp4", Some("m4a")) | ("webm", Some("webm"))
    )
}

fn ffmpeg(root: &Path) -> Result<PathBuf, &'static str> {
    let tool = tools::Tool::Ffmpeg;
    match tools::probe(root, tool).state {
        tools::State::Ready => tools::executable(root, tool).map_err(|_| "FFMPEG_FAILED"),
        tools::State::Missing => Err("FFMPEG_MISSING"),
        tools::State::Unsupported => Err("FFMPEG_UNSUPPORTED"),
        _ => Err("FFMPEG_FAILED"),
    }
}

fn remux(
    ffmpeg: &Path,
    video: &Path,
    audio: Option<&Path>,
    output: &Path,
    control: &Control,
) -> Result<(), &'static str> {
    folders::check_path(output)?;
    let mut command = Command::new(ffmpeg);
    command.args(["-nostdin", "-v", "error", "-xerror", "-n"]);
    for input in [Some(video), audio].into_iter().flatten() {
        folders::check_path(input)?;
        let demuxer = match input.extension().and_then(|ext| ext.to_str()) {
            Some("mp4" | "m4a") => "mov",
            Some("webm") => "matroska",
            _ => return Err("DOWNLOAD_FORMAT_UNSUPPORTED"),
        };
        command
            .args(["-protocol_whitelist", "file", "-f", demuxer, "-i"])
            .arg(input);
    }
    command
        .args([
            "-map",
            "0:V:0",
            "-map",
            if audio.is_some() { "1:a:0" } else { "0:a:0?" },
            "-map_metadata",
            "-1",
            "-map_chapters",
            "-1",
            "-c",
            "copy",
        ])
        .arg(output);
    let result = process::run_with(
        &mut command,
        TRANSFER_TIMEOUT,
        (4096, 64 * 1024),
        None,
        |_| {},
        || control.interrupted(),
    )
    .map_err(|error| match error {
        process::RunError::Interrupted => "DOWNLOAD_INTERRUPTED",
        process::RunError::Timeout => "DOWNLOAD_TIMEOUT",
        _ => "FFMPEG_FAILED",
    })?;
    if !result.success
        || !fs::metadata(output).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
    {
        return Err("CONTAINER_INCOMPATIBLE");
    }
    Ok(())
}

fn choose_video<'a>(
    formats: impl Iterator<Item = &'a extract::Format>,
    quality: &Quality,
) -> Result<&'a extract::Format, &'static str> {
    let cap = match quality {
        Quality::Best => None,
        Quality::Q2160 => Some(2160),
        Quality::Q1080 => Some(1080),
        Quality::Q720 => Some(720),
    };
    let mut formats = formats.peekable();
    if formats.peek().is_none() {
        return Err("DOWNLOAD_FORMAT_UNSUPPORTED");
    }
    formats
        .filter(|format| cap.is_none_or(|cap| format.height.is_some_and(|height| height <= cap)))
        .max_by_key(|format| {
            (
                format.height.unwrap_or(0),
                format.width.unwrap_or(0),
                format.tracks == extract::Tracks::AudioVideo,
            )
        })
        .ok_or("QUALITY_UNAVAILABLE")
}

fn transfer(
    analyzer: &extract::Analyzer,
    format: &extract::Format,
    path: &Path,
    control: &Control,
    mut update: impl FnMut(Progress) + Send + 'static,
) -> Result<(), &'static str> {
    folders::check_path(path)?;
    let media = format.media.as_ref().ok_or("DOWNLOAD_FORMAT_UNSUPPORTED")?;
    let proxy = proxy::Proxy::download(TRANSFER_TIMEOUT).map_err(|_| "NETWORK_ERROR")?;
    // Construct a new info object. Never pass through extractor metadata, headers, or file paths.
    let input = serde_json::to_vec(&serde_json::json!({
        "id": "public-video",
        "title": "Public video",
        "url": media.url.as_str(),
        "protocol": media.protocol,
        "ext": format.container,
        "acodec": format.audio_codec,
        "format_id": "approved",
        "vcodec": if format.tracks == extract::Tracks::AudioOnly { Some("none") } else { format.video_codec.as_deref() },
    }))
    .map_err(|_| "DOWNLOAD_FAILED")?;
    let mut command = analyzer.command(&proxy);
    let template = path.to_str().ok_or("INVALID_DIRECTORY")?.replace('%', "%%");
    command.args([
        "--load-info-json", "-", "--format", "approved", "--downloader", "native", "--fixup", "never",
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
        || control.interrupted(),
    );
    if proxy.blocked() {
        return Err("URL_NOT_ALLOWED");
    }
    let output = output.map_err(|error| match error {
        process::RunError::Interrupted => "DOWNLOAD_INTERRUPTED",
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
    use crate::settings::{Settings, Store};

    #[test]
    fn names_are_safe_and_atomic_publication_preserves_existing_files() {
        let dir = tempfile::tempdir().unwrap();
        let addr = "https://example.org/watch?v=one";
        let name = |title, rule| filename_stem(title, addr, rule, dir.path(), "mp4").unwrap();
        assert_eq!(name(" .. ", &Filename::Title), "video");
        assert_eq!(name("COM².txt", &Filename::Title), "_COM².txt");
        assert_eq!(
            name("한글/영상?value=secret", &Filename::Title),
            "한글_영상"
        );
        let id = name("영상", &Filename::TitleId);
        assert_eq!(
            id,
            filename_stem(
                "영상",
                "https://example.org/watch?v=two",
                &Filename::TitleId,
                dir.path(),
                "mp4"
            )
            .unwrap()
        );
        assert!(name("영상", &Filename::DateTitle).ends_with("-영상"));
        let long = name(&"한".repeat(300), &Filename::Title);
        assert!(long.len() < 120);
        let first = dir.path().join("first.tmp");
        let second = dir.path().join("second.tmp");
        fs::write(&first, b"one").unwrap();
        fs::write(&second, b"two").unwrap();
        fs::write(dir.path().join("영상.mp4"), b"existing").unwrap();
        let paths = std::thread::scope(|threads| {
            let a = threads.spawn(|| publish(&first, dir.path(), "영상", "mp4").unwrap());
            let b = threads.spawn(|| publish(&second, dir.path(), "영상", "mp4").unwrap());
            [a.join().unwrap(), b.join().unwrap()]
        });
        assert_ne!(paths[0], paths[1]);
        assert_eq!(fs::read(dir.path().join("영상.mp4")).unwrap(), b"existing");
        assert_eq!(fs::read(dir.path().join(&paths[0])).unwrap(), b"one");
        assert_eq!(fs::read(dir.path().join(&paths[1])).unwrap(), b"two");
    }

    #[test]
    fn selects_highest_eligible_height_without_exceeding_the_limit() {
        let make = |height, tracks| extract::Format {
            id: String::new(),
            container: None,
            width: None,
            height,
            video_codec: None,
            audio_codec: None,
            bitrate: None,
            tracks,
            media: None,
        };
        let formats = [
            make(None, extract::Tracks::Unknown),
            make(Some(2160), extract::Tracks::AudioVideo),
            make(Some(1080), extract::Tracks::AudioVideo),
            make(Some(720), extract::Tracks::AudioVideo),
            make(Some(720), extract::Tracks::VideoOnly),
        ];
        for (quality, height) in [
            (Quality::Best, 2160),
            (Quality::Q2160, 2160),
            (Quality::Q1080, 1080),
            (Quality::Q720, 720),
        ] {
            let chosen = choose_video(formats.iter(), &quality).unwrap();
            assert_eq!(chosen.height, Some(height));
            assert_eq!(chosen.tracks, extract::Tracks::AudioVideo);
        }
        assert_eq!(
            choose_video(formats[0..3].iter(), &Quality::Q720).err(),
            Some("QUALITY_UNAVAILABLE")
        );
        assert_eq!(
            choose_video(formats[2..].iter(), &Quality::Q2160)
                .unwrap()
                .height,
            Some(1080)
        );
    }

    #[test]
    #[ignore = "requires verified developer binaries and public network access"]
    fn real_quality_limit_selects_lower_public_dash_variant() {
        let root = PathBuf::from(std::env::var_os("DM_R4_TOOL_ROOT").unwrap());
        let report = extract::Analyzer::open(&root)
            .unwrap()
            .analyze("https://storage.googleapis.com/shaka-demo-assets/sintel/dash.mpd")
            .unwrap();
        let select = |quality| {
            choose_video(
                report.formats.iter().filter(|format| {
                    matches!(
                        format.tracks,
                        extract::Tracks::AudioVideo | extract::Tracks::VideoOnly
                    )
                }),
                quality,
            )
            .unwrap()
            .height
            .unwrap()
        };
        assert!(select(&Quality::Best) > select(&Quality::Q720));
        assert!(select(&Quality::Q720) <= 720);
        println!(
            "QUALITY_SELECTION_PASS: best={}p, 720p limit={}p",
            select(&Quality::Best),
            select(&Quality::Q720)
        );
    }

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
        let mut store = Store::new(
            output.path().to_path_buf(),
            output.path().join("Downloads").to_str().unwrap().into(),
        );
        let mut settings = store.load().unwrap().settings;
        let directory = output.path().join("저장 위치");
        settings.download_directory = directory.to_str().unwrap().into();
        settings.filename = Filename::TitleId;
        store.save(settings.clone()).unwrap();
        let settings = Store::new(output.path().to_path_buf(), String::new())
            .current_settings()
            .unwrap();
        let (send, receive) = std::sync::mpsc::channel();
        let result = download(
            &Work {
                root: root.clone(),
                settings,
                page: "https://interactive-examples.mdn.mozilla.net/media/cc0-videos/flower.mp4"
                    .into(),
                media: None,
            },
            &Control::default(),
            move |value| {
                if let Update::Progress(value) = value {
                    send.send(value).unwrap();
                }
            },
        )
        .unwrap();
        assert!(result.filename.starts_with("flower-"));
        let file = directory.join(result.filename);
        assert!(fs::metadata(&file).unwrap().len() > 0);
        assert!(receive
            .try_iter()
            .any(|value| value.downloaded > 0 && value.total.is_some()));
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        decode(&root, &file, false);
        println!("MP4_DOWNLOAD_AND_DECODE_PASS");
    }

    #[test]
    #[ignore = "requires verified developer binaries and public network access"]
    fn real_dash_merges_capped_video_and_audio() {
        let root = PathBuf::from(std::env::var_os("DM_R4_TOOL_ROOT").unwrap());
        let output = tempfile::tempdir().unwrap();
        let mut settings = Settings::defaults(output.path().to_str().unwrap().into());
        settings.quality = Quality::Q720;
        settings.format = Container::Mp4;
        let result = download(
            &Work {
                root: root.clone(),
                settings,
                page: "https://storage.googleapis.com/shaka-demo-assets/sintel/dash.mpd".into(),
                media: None,
            },
            &Control::default(),
            |_| {},
        )
        .unwrap();
        assert_eq!(result.height, Some(546));
        assert!(result.filename.ends_with(".mp4"));
        decode(&root, &output.path().join(result.filename), true);
        println!("DASH_DOWNLOAD_AND_MERGE_PASS: 546p with audio");
    }

    fn decode(root: &Path, file: &Path, audio: bool) {
        let mut command = Command::new(root.join("tools/ffmpeg.exe"));
        command
            .args(["-nostdin", "-v", "error", "-xerror", "-i"])
            .arg(file)
            .args(["-map", "0:V:0"]);
        if audio {
            command.args(["-map", "0:a:0"]);
        }
        let output = process::run(
            command.args(["-f", "null", "-"]),
            Duration::from_secs(30),
            4096,
            4096,
        )
        .unwrap();
        assert!(output.success, "FFmpeg decode failed");
    }
}
