use crate::analysis::{extract, process, proxy, tools};
use crate::{
    folders,
    settings::{Filename, Format as Container, Quality},
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Read},
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    time::Duration,
};
use windows_sys::Win32::Storage::FileSystem::MoveFileExW;

mod dash;
mod hls;
pub(crate) mod notification;
pub mod queue;
mod stream;
use queue::{Control, Work};
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    downloaded: u64,
    total: Option<u64>,
    bytes_per_second: Option<f64>,
    segments: Option<(usize, usize)>,
    continuation: Option<Continuation>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
enum Continuation {
    Resumed,
    Restarted,
}

struct Saved {
    filename: String,
    directory: PathBuf,
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
                && supported_transfer(format)
                && (settings.format != Container::Mp4 || format.container.as_deref() == Some("mp4"))
        }),
        &settings.quality,
    )?;
    let ext = format
        .container
        .as_deref()
        .ok_or("DOWNLOAD_FORMAT_UNSUPPORTED")?;
    let hls = format.media.as_ref().is_some_and(extract::Media::is_hls);
    let mut streams = hls
        .then(|| stream::StreamClient::new(control))
        .transpose()?;
    let audio = if let Some(streams) = &streams {
        hls::audio(streams, format, &report.formats)?
    } else if format.tracks == extract::Tracks::VideoOnly {
        report
            .formats
            .iter()
            .filter(|candidate| {
                candidate.tracks == extract::Tracks::AudioOnly
                    && dash_or_direct(candidate)
                    && same_manifest(format, candidate)
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
            candidate.tracks == extract::Tracks::AudioOnly
                && dash_or_direct(candidate)
                && same_manifest(format, candidate)
        })
    {
        return Err("CONTAINER_INCOMPATIBLE");
    }
    let dash_hash = if let Some(dash) = [Some(format), audio]
        .into_iter()
        .flatten()
        .filter_map(|format| format.media.as_ref())
        .find(|media| media.is_dash())
    {
        let client = stream::StreamClient::new(control)?;
        let hash = dash::check_manifest(&client, dash)?;
        streams = Some(client);
        Some(hash)
    } else {
        None
    };
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
    let mut partial = work.partial.lock().map_err(|_| "DOWNLOAD_FAILED")?;
    if audio.is_none() {
        partial[1] = stream::TrackPartial::default();
    }
    let video = staging.path().join(format!("source.{ext}"));
    let update = Arc::new(update);
    update(Update::Downloading {
        title: report.title,
        height: format.height,
    });
    let video_update = Arc::clone(&update);
    let (video, video_demux) = transfer(
        streams.as_ref(),
        format,
        &video,
        &directory,
        &mut partial[0],
        dash_hash,
        audio.is_none(),
        control,
        move |value| video_update(Update::Progress(value)),
    )?;
    let audio_file = audio
        .map(|audio| -> Result<(PathBuf, Demux), &'static str> {
            let ext = audio
                .container
                .as_deref()
                .ok_or("DOWNLOAD_FORMAT_UNSUPPORTED")?;
            let file = staging.path().join(format!("audio.{ext}"));
            let audio_update = Arc::clone(&update);
            transfer(
                streams.as_ref(),
                audio,
                &file,
                &directory,
                &mut partial[1],
                dash_hash,
                false,
                control,
                move |value| audio_update(Update::Progress(value)),
            )
        })
        .transpose()?;
    update(Update::Merging);
    let path = staging.path().join(format!("finished.{output_ext}"));
    let audio = audio_file
        .as_ref()
        .map(|(file, demux)| (file.as_path(), *demux));
    remux(&ffmpeg, (&video, video_demux), audio, &path, control)?;
    let filename = control.commit(|| publish(&path, &directory, &stem, output_ext))?;
    Ok(Saved {
        filename,
        directory,
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

fn supported_transfer(format: &extract::Format) -> bool {
    dash_or_direct(format) || format.media.as_ref().is_some_and(extract::Media::is_hls)
}

fn dash_or_direct(format: &extract::Format) -> bool {
    direct_transfer(format) || format.media.as_ref().is_some_and(extract::Media::is_dash)
}

fn same_manifest(video: &extract::Format, audio: &extract::Format) -> bool {
    let manifest = |format: &extract::Format| {
        format
            .media
            .as_ref()
            .and_then(|media| media.manifest.clone())
    };
    manifest(video) == manifest(audio)
}

fn same_family(video_ext: &str, audio: &extract::Format) -> bool {
    matches!(
        (video_ext, audio.container.as_deref()),
        ("mp4", Some("m4a" | "mp4")) | ("webm", Some("webm"))
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

#[derive(Clone, Copy)]
enum Demux {
    Mov,
    Matroska,
    HlsPacked,
}
impl Demux {
    fn input_format(self, path: &Path) -> Result<&'static str, &'static str> {
        match self {
            Self::Mov => Ok("mov"),
            Self::Matroska => Ok("matroska"),
            Self::HlsPacked => {
                let mut input = fs::File::open(path).map_err(|_| "DOWNLOAD_SAVE_FAILED")?;
                let mut prefix = [0; 377];
                let mut length = 0;
                while length < prefix.len() {
                    let read = input
                        .read(&mut prefix[length..])
                        .map_err(|_| "DOWNLOAD_SAVE_FAILED")?;
                    if read == 0 {
                        break;
                    }
                    length += read;
                }
                if length == prefix.len()
                    && [0, 188, 376].iter().all(|offset| prefix[*offset] == 0x47)
                {
                    Ok("mpegts")
                } else {
                    Ok("aac")
                }
            }
        }
    }
}

fn remux(
    ffmpeg: &Path,
    video: (&Path, Demux),
    audio: Option<(&Path, Demux)>,
    output: &Path,
    control: &Control,
) -> Result<(), &'static str> {
    folders::check_path(output)?;
    let mut command = Command::new(ffmpeg);
    command.args(["-nostdin", "-v", "error", "-xerror", "-n"]);
    for (input, demux) in [Some(video), audio].into_iter().flatten() {
        folders::check_path(input)?;
        let demuxer = demux.input_format(input)?;
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
    if !result.success {
        let stderr = String::from_utf8_lossy(&result.stderr).to_ascii_lowercase();
        if [
            "invalid data found when processing input",
            "could not find codec parameters",
            "matches no streams",
            "moov atom not found",
        ]
        .iter()
        .any(|marker| stderr.contains(marker))
        {
            return Err("MEDIA_INVALID");
        }
        return Err("CONTAINER_INCOMPATIBLE");
    }
    if !fs::metadata(output).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0) {
        return Err("DOWNLOAD_SAVE_FAILED");
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
    streams: Option<&stream::StreamClient<'_>>,
    format: &extract::Format,
    path: &Path,
    directory: &Path,
    partial: &mut stream::TrackPartial,
    dash_hash: Option<[u8; 32]>,
    keep_on_exit: bool,
    control: &Control,
    update: impl FnMut(Progress) + Send + 'static,
) -> Result<(PathBuf, Demux), &'static str> {
    folders::check_path(path)?;
    let media = format.media.as_ref().ok_or("DOWNLOAD_FORMAT_UNSUPPORTED")?;
    if media.is_hls() {
        partial.file.take();
        let streams = streams.ok_or("DOWNLOAD_FAILED")?;
        let (base, text) = streams.playlist(&media.url)?;
        let (parts, demux) = hls::plan(&base, &text)?;
        let plan = stream::SegmentPlan {
            source: base,
            format: format.id.clone(),
            playlist_hash: Sha256::digest(text.as_bytes()).into(),
            parts,
        };
        let path = streams.transfer(plan, directory, &mut partial.segments, update)?;
        return Ok((path, demux));
    }
    if media.is_dash() {
        partial.file.take();
        let demux = match format.container.as_deref() {
            Some("mp4" | "m4a") => Demux::Mov,
            Some("webm") => Demux::Matroska,
            _ => return Err("DOWNLOAD_FORMAT_UNSUPPORTED"),
        };
        let parts = media
            .dash_segments
            .iter()
            .map(|url| stream::Part {
                url: url.clone(),
                range: None,
            })
            .collect();
        let plan = stream::SegmentPlan {
            source: media.url.clone(),
            format: format.id.clone(),
            playlist_hash: dash_hash.ok_or("DASH_MANIFEST_UNSUPPORTED")?,
            parts,
        };
        let path = streams.ok_or("DOWNLOAD_FAILED")?.transfer(
            plan,
            directory,
            &mut partial.segments,
            update,
        )?;
        return Ok((path, demux));
    }
    partial.segments.take();
    stream::StreamClient::new(control)?.download_file(
        &media.url,
        path,
        directory,
        &mut partial.file,
        keep_on_exit,
        update,
    )?;
    match format.container.as_deref() {
        Some("mp4" | "m4a") => Ok((path.to_path_buf(), Demux::Mov)),
        Some("webm") => Ok((path.to_path_buf(), Demux::Matroska)),
        _ => Err("DOWNLOAD_FORMAT_UNSUPPORTED"),
    }
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
                partial: std::sync::Mutex::default(),
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
                partial: std::sync::Mutex::default(),
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

    #[test]
    #[ignore = "requires verified developer binaries and public network access"]
    fn real_hls_saves_selected_video_and_default_audio() {
        let root = PathBuf::from(std::env::var_os("DM_R4_TOOL_ROOT").unwrap());
        let output = tempfile::tempdir().unwrap();
        let mut settings = Settings::defaults(output.path().to_str().unwrap().into());
        settings.quality = Quality::Q720;
        let (send, receive) = std::sync::mpsc::channel();
        let result = download(
            &Work {
                root: root.clone(),
                settings,
                page: "https://storage.googleapis.com/shaka-demo-assets/angel-one-hls/hls.m3u8"
                    .into(),
                media: None,
                partial: std::sync::Mutex::default(),
            },
            &Control::default(),
            move |event| {
                if let Update::Progress(progress) = event {
                    send.send(progress).unwrap();
                }
            },
        )
        .unwrap();
        assert_eq!(result.height, Some(576));
        assert!(result.filename.ends_with(".mp4"));
        assert!(receive.try_iter().any(|progress| progress
            .segments
            .is_some_and(|(done, total)| done == total && total > 1)));
        decode(&root, &output.path().join(result.filename), true);
        println!("HLS_DOWNLOAD_AND_DECODE_PASS: 576p with audio");
    }

    #[test]
    #[ignore = "requires verified developer binaries and downloads a public DASH sample"]
    fn real_segmented_dash_saves_and_decodes_both_tracks() {
        let root = PathBuf::from(std::env::var_os("DM_R4_TOOL_ROOT").unwrap());
        let output = tempfile::tempdir().unwrap();
        let mut settings = Settings::defaults(output.path().to_str().unwrap().into());
        settings.quality = Quality::Q720;
        settings.format = Container::Mp4;
        let (send, receive) = std::sync::mpsc::channel();
        let result = download(
            &Work {
                root: root.clone(),
                settings,
                page: "https://dash.akamaized.net/akamai/bbb_30fps/bbb_30fps.mpd".into(),
                media: None,
                partial: std::sync::Mutex::default(),
            },
            &Control::default(),
            move |event| {
                if let Update::Progress(progress) = event {
                    send.send(progress).unwrap();
                }
            },
        )
        .unwrap();
        assert_eq!(result.height, Some(720));
        assert_eq!(
            receive
                .try_iter()
                .filter(|progress| progress.segments == Some((160, 160)))
                .count(),
            2
        );
        let output_file = output.path().join(result.filename);
        let mut command = Command::new(root.join("tools/ffmpeg.exe"));
        command
            .args(["-nostdin", "-v", "error", "-xerror", "-i"])
            .arg(output_file)
            .args([
                "-map", "0:V:0", "-map", "0:a:0", "-t", "5", "-f", "null", "-",
            ]);
        assert!(
            process::run(&mut command, Duration::from_secs(30), 4096, 4096)
                .unwrap()
                .success
        );
        println!("SEGMENTED_DASH_PASS: 720p with audio, 160 segments per track");
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
