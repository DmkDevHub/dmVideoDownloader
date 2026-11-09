use super::{
    failure,
    process::{self, RunError},
    proxy,
    tools::{self, State, Tool},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

#[derive(Deserialize)]
struct RawReport {
    title: Option<String>,
    webpage_url: Option<String>,
    has_drm: Option<serde_json::Value>,
    #[serde(default)]
    formats: Vec<RawFormat>,
}
#[derive(Deserialize)]
struct RawFormat {
    format_id: Option<String>,
    url: Option<String>,
    manifest_url: Option<String>,
    ext: Option<String>,
    width: Option<i64>,
    height: Option<i64>,
    vcodec: Option<String>,
    acodec: Option<String>,
    has_drm: Option<serde_json::Value>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Format {
    pub id: String,
    pub container: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub tracks: Tracks,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Tracks {
    AudioVideo,
    VideoOnly,
    AudioOnly,
    Unknown,
}

enum Codec {
    Named(String),
    Absent,
    Unknown,
}
impl Codec {
    fn parse(value: Option<String>) -> Self {
        match format_tag(value, 64)
            .map(|value| value.to_ascii_lowercase())
            .as_deref()
        {
            Some("none") => Self::Absent,
            Some("unknown") | None => Self::Unknown,
            Some(name) => Self::Named(name.to_owned()),
        }
    }
    fn name(self) -> Option<String> {
        if let Self::Named(name) = self {
            Some(name)
        } else {
            None
        }
    }
}
fn dimension(value: Option<i64>) -> Option<u32> {
    value
        .filter(|value| (1..=32_768).contains(value))
        .map(|value| value as u32)
}
fn normalize(raw: RawFormat) -> Option<Format> {
    let video = Codec::parse(raw.vcodec);
    let audio = Codec::parse(raw.acodec);
    let container = format_tag(raw.ext, 16).map(|value| value.to_ascii_lowercase());
    let tracks = match (&video, &audio) {
        (Codec::Named(_), Codec::Named(_)) => Tracks::AudioVideo,
        (Codec::Named(_), Codec::Absent) => Tracks::VideoOnly,
        (Codec::Absent, Codec::Named(_)) => Tracks::AudioOnly,
        (Codec::Absent, Codec::Absent) => return None,
        (Codec::Absent | Codec::Unknown, _)
            if matches!(
                container.as_deref(),
                Some("mp3" | "m4a" | "aac" | "opus" | "wav" | "flac" | "oga" | "wma" | "mka")
            ) =>
        {
            Tracks::AudioOnly
        }
        _ => Tracks::Unknown,
    };
    Some(Format {
        id: format_tag(raw.format_id, 96)?,
        container,
        width: dimension(raw.width),
        height: dimension(raw.height),
        video_codec: video.name(),
        audio_codec: audio.name(),
        tracks,
    })
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub title: String,
    pub formats: Vec<Format>,
    pub source: Source,
    pub excluded_protected: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    Direct,
    Page,
    Media,
}

fn format_tag(value: Option<String>, max: usize) -> Option<String> {
    value.filter(|value| {
        !value.is_empty()
            && value.len() <= max
            && value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'+')
            })
    })
}
fn report(bytes: &[u8]) -> Result<Report, &'static str> {
    let raw: RawReport = serde_json::from_slice(bytes).map_err(|_| "INVALID_METADATA")?;
    let protected = |flag: Option<&serde_json::Value>| match flag {
        Some(serde_json::Value::Bool(true)) => Some("DRM_PROTECTED"),
        Some(serde_json::Value::String(value)) if value.eq_ignore_ascii_case("maybe") => {
            Some("POSSIBLY_PROTECTED")
        }
        _ => None,
    };
    if let Some(reason) = protected(raw.has_drm.as_ref()) {
        return Err(reason);
    }
    if raw
        .webpage_url
        .as_deref()
        .is_some_and(|url| proxy::target(url).is_none())
    {
        return Err("URL_NOT_ALLOWED");
    }
    let mut excluded_protected = 0;
    let mut confirmed_protection = false;
    let mut excluded_address = false;
    let title = raw
        .title
        .as_deref()
        .filter(|title| !title.contains("://"))
        .map(|title| {
            title
                .chars()
                .filter(|character| !character.is_control())
                .take(200)
                .collect::<String>()
        })
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| "공개 영상".into());
    let mut ids = HashSet::new();
    let mut formats: Vec<_> = raw
        .formats
        .into_iter()
        .filter(|format| {
            if let Some(reason) = protected(format.has_drm.as_ref()) {
                excluded_protected += 1;
                confirmed_protection |= reason == "DRM_PROTECTED";
                false
            } else {
                let allowed = [format.url.as_deref(), format.manifest_url.as_deref()]
                    .into_iter()
                    .flatten()
                    .all(|url| proxy::target(url).is_some());
                excluded_address |= !allowed;
                allowed
            }
        })
        .filter_map(normalize)
        .filter(|format| ids.insert(format.id.clone()))
        .take(256)
        .collect();
    if formats
        .iter()
        .all(|format| format.tracks == Tracks::AudioOnly)
    {
        if confirmed_protection {
            return Err("DRM_PROTECTED");
        }
        if excluded_protected > 0 {
            return Err("POSSIBLY_PROTECTED");
        }
        if excluded_address {
            return Err("URL_NOT_ALLOWED");
        }
        return Err("NO_FORMATS");
    }
    formats.sort_by_key(|format| std::cmp::Reverse(format.height.unwrap_or(0)));
    Ok(Report {
        title,
        formats,
        source: Source::Direct,
        excluded_protected,
    })
}

pub fn candidate(
    page: &str,
    media: Option<&str>,
    mut attempt: impl FnMut(&str) -> Result<Report, &'static str>,
) -> Result<Report, &'static str> {
    match attempt(page) {
        Ok(mut report) => {
            report.source = Source::Page;
            Ok(report)
        }
        Err(reason @ ("UNSUPPORTED_URL" | "NO_FORMATS")) => {
            let media = media.filter(|address| *address != page).ok_or(reason)?;
            if proxy::target(media).is_none() {
                return Err("URL_NOT_ALLOWED");
            }
            let mut report = attempt(media)?;
            report.source = Source::Media;
            Ok(report)
        }
        Err(error) => Err(error),
    }
}

pub struct Analyzer {
    exe: PathBuf,
}
impl Analyzer {
    pub fn open(root: &Path) -> Result<Self, &'static str> {
        let status = tools::probe(root, Tool::YtDlp);
        match status.state {
            State::Ready => {}
            State::Missing => return Err("YT_DLP_MISSING"),
            State::Unsupported => return Err("YT_DLP_UNSUPPORTED"),
            State::Timeout => return Err("ANALYSIS_TIMEOUT"),
            _ => return Err("YT_DLP_FAILED"),
        }
        let exe = tools::executable(root, Tool::YtDlp).map_err(|_| "YT_DLP_FAILED")?;
        Ok(Self { exe })
    }

    pub fn analyze(&self, address: &str) -> Result<Report, &'static str> {
        let address = proxy::target(address).ok_or("URL_NOT_ALLOWED")?;
        let proxy = proxy::Proxy::start().map_err(|_| "ANALYSIS_FAILED")?;
        self.run(address, proxy)
    }

    fn run(&self, address: url::Url, proxy: proxy::Proxy) -> Result<Report, &'static str> {
        let mut command = Command::new(&self.exe);
        command
            .args([
                "--ignore-config",
                "--no-plugin-dirs",
                "--no-cache-dir",
                "--no-js-runtimes",
                "--no-remote-components",
                "--no-cookies",
                "--no-cookies-from-browser",
                "--no-mark-watched",
                "--no-playlist",
                "--playlist-items",
                "1",
                "--use-extractors",
                "generic,html5",
                "--simulate",
                "--no-check-formats",
                "--dump-single-json",
                "--no-progress",
                "--color",
                "never",
                "--socket-timeout",
                "7",
                "--retries",
                "0",
                "--extractor-retries",
                "0",
                "--proxy",
            ])
            .arg(proxy.endpoint())
            .arg("--")
            .arg(address.as_str());
        let output = process::run(
            &mut command,
            Duration::from_secs(30),
            4 * 1024 * 1024,
            64 * 1024,
        );
        if proxy.blocked() {
            return Err("URL_NOT_ALLOWED");
        }
        let output = output.map_err(|error| match error {
            RunError::Timeout => "ANALYSIS_TIMEOUT",
            RunError::OutputLimit => "METADATA_LIMIT",
            _ => "YT_DLP_FAILED",
        })?;
        if !output.success {
            return Err(failure::classify(&output.stderr));
        }
        report(&output.stdout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn projects_only_safe_metadata() {
        let result = report(br#"{"title":"Video","formats":[{"format_id":"18","ext":"mp4","height":720,"url":"https://example.org/video.mp4"},{"format_id":"19","url":"https://example.org/?token=secret"}],"http_headers":{"Cookie":"secret"}}"#).unwrap();
        let json = serde_json::to_value(result).unwrap();
        assert_eq!(json["formats"][0]["height"], 720);
        assert_eq!(json["formats"].as_array().unwrap().len(), 1);
        assert!(!json.to_string().contains("secret"));
        assert!(matches!(
            report(
                br#"{"formats":[{"format_id":"19","manifest_url":"http://127.0.0.1/video.mpd"}]}"#
            ),
            Err("URL_NOT_ALLOWED")
        ));
        assert!(matches!(report(br#"{"formats":[]}"#), Err("NO_FORMATS")));
    }

    #[test]
    fn protected_tracks_never_appear_in_the_report() {
        assert!(matches!(
            report(br#"{"has_drm":true,"formats":[{"format_id":"v"}]}"#),
            Err("DRM_PROTECTED")
        ));
        assert!(matches!(
            report(br#"{"formats":[{"format_id":"v","has_drm":"maybe"}]}"#),
            Err("POSSIBLY_PROTECTED")
        ));
        let mixed = report(br#"{"formats":[{"format_id":"clear","ext":"mp4"},{"format_id":"protected","has_drm":true}]}"#).unwrap();
        assert_eq!(mixed.excluded_protected, 1);
        assert_eq!(mixed.formats[0].id, "clear");
        assert_eq!(mixed.formats.len(), 1);
    }

    #[test]
    #[ignore = "requires verified developer binaries and public network access"]
    fn real_public_error_responses() {
        let root = PathBuf::from(std::env::var_os("DM_R4_TOOL_ROOT").unwrap());
        let analyzer = Analyzer::open(&root).unwrap();
        for (address, expected) in [
            ("https://httpbin.org/status/401", "AUTH_REQUIRED"),
            ("https://httpbin.org/status/403", "ACCESS_DENIED"),
            ("https://httpbin.org/status/410", "EXPIRED_OR_GONE"),
            (
                "https://storage.googleapis.com/shaka-demo-assets/angel-one-clearkey/dash.mpd",
                "DRM_PROTECTED",
            ),
        ] {
            assert_eq!(
                analyzer.analyze(address).err(),
                Some(expected),
                "fixture for {expected}"
            );
            println!("PUBLIC_FAILURE_PASS: {expected}");
        }
    }

    #[test]
    #[ignore = "requires verified developer binaries"]
    fn real_redirect_cannot_reach_loopback() {
        use std::io::{BufRead, BufReader, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let socket = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut connection = BufReader::new(stream);
            let mut line = String::new();
            while connection.read_line(&mut line).unwrap() != 0 && line != "\r\n" {
                line.clear();
            }
            write!(connection.get_mut(), "HTTP/1.1 302 Found\r\nLocation: http://{socket}/private\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
            listener
        });
        let root = PathBuf::from(std::env::var_os("DM_R4_TOOL_ROOT").unwrap());
        let analyzer = Analyzer::open(&root).unwrap();
        let outcome = analyzer.run(
            proxy::target("http://fixture.example.org/page").unwrap(),
            proxy::Proxy::with_test_fixture(socket).unwrap(),
        );
        assert_eq!(outcome.err(), Some("URL_NOT_ALLOWED"));
        let listener = server.join().unwrap();
        listener.set_nonblocking(true).unwrap();
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        println!("REDIRECT_BLOCKED_PASS");
    }

    #[test]
    fn normalizes_split_muxed_and_unknown_tracks_without_inventing_dimensions() {
        let formats = report(br#"{"formats":[
            {"format_id":"audio","ext":"M4A","vcodec":"none","acodec":"mp4a.40.2"},
            {"format_id":"video","ext":"MP4","width":1920,"height":1080,"vcodec":"AVC1.640028","acodec":"none"},
            {"format_id":"both","height":720,"vcodec":"vp9","acodec":"opus"},
            {"format_id":"unknown","width":-1,"height":999999,"vcodec":"https://example.org/?token=secret"},
            {"format_id":"both","height":2160},
            {"format_id":"discard","vcodec":"none","acodec":"none"}
        ]}"#).unwrap().formats;
        assert_eq!(
            formats
                .iter()
                .map(|format| format.tracks)
                .collect::<Vec<_>>(),
            [
                Tracks::VideoOnly,
                Tracks::AudioVideo,
                Tracks::AudioOnly,
                Tracks::Unknown
            ]
        );
        assert_eq!(
            (formats[0].width, formats[0].height),
            (Some(1920), Some(1080))
        );
        assert_eq!(formats[0].video_codec.as_deref(), Some("avc1.640028"));
        assert_eq!(formats[2].container.as_deref(), Some("m4a"));
        assert!(
            formats[3].width.is_none()
                && formats[3].height.is_none()
                && formats[3].video_codec.is_none()
        );
        assert!(matches!(
            report(br#"{"formats":[{"format_id":"audio","ext":"mp3"}]}"#),
            Err("NO_FORMATS")
        ));
    }

    #[test]
    #[ignore = "requires verified developer binaries and public network access"]
    fn real_split_stream_formats() {
        let root = PathBuf::from(std::env::var_os("DM_R4_TOOL_ROOT").unwrap());
        let report = Analyzer::open(&root)
            .unwrap()
            .analyze("https://storage.googleapis.com/shaka-demo-assets/angel-one/dash.mpd")
            .unwrap();
        assert!(report
            .formats
            .iter()
            .any(|format| format.tracks == Tracks::VideoOnly && format.height.is_some()));
        assert!(report
            .formats
            .iter()
            .any(|format| format.tracks == Tracks::AudioOnly && format.audio_codec.is_some()));
        println!("SPLIT_FORMATS_PASS: {} format(s)", report.formats.len());
    }

    #[test]
    fn candidate_only_falls_back_for_unsupported_pages_or_missing_formats() {
        let page = "https://example.org/page";
        let media = "https://example.org/video.mp4";
        for reason in [
            "UNSUPPORTED_URL",
            "NO_FORMATS",
            "AUTH_REQUIRED",
            "DRM_PROTECTED",
            "POSSIBLY_PROTECTED",
            "EXPIRED_OR_GONE",
            "URL_NOT_ALLOWED",
            "ANALYSIS_TIMEOUT",
            "ANALYSIS_FAILED",
        ] {
            let mut calls = Vec::new();
            let result = candidate(page, Some(media), |address| {
                calls.push(address.to_owned());
                if address == page {
                    Err(reason)
                } else {
                    report(br#"{"formats":[{"format_id":"1"}]}"#)
                }
            });
            if matches!(reason, "UNSUPPORTED_URL" | "NO_FORMATS") {
                assert_eq!(result.unwrap().source, Source::Media);
                assert_eq!(calls, [page, media]);
            } else {
                assert_eq!(result.err(), Some(reason));
                assert_eq!(calls, [page]);
            }
        }
        let result = candidate(page, Some(media), |address| {
            assert_eq!(address, page);
            report(br#"{"formats":[{"format_id":"1"}]}"#)
        });
        assert_eq!(result.unwrap().source, Source::Page);
        assert_eq!(
            candidate(page, Some("http://127.0.0.1/x"), |_| Err("NO_FORMATS")).err(),
            Some("URL_NOT_ALLOWED")
        );
    }

    #[test]
    #[ignore = "requires verified developer binaries and public network access"]
    fn real_candidate_media_fallback() {
        let root = PathBuf::from(std::env::var_os("DM_R4_TOOL_ROOT").unwrap());
        let analyzer = Analyzer::open(&root).unwrap();
        let result = candidate(
            "https://example.org/",
            Some("https://interactive-examples.mdn.mozilla.net/media/cc0-videos/flower.mp4"),
            |address| analyzer.analyze(address),
        )
        .unwrap();
        assert_eq!(result.source, Source::Media);
        assert!(result
            .formats
            .iter()
            .any(|format| format.container.as_deref() == Some("mp4")));
        println!("CANDIDATE_FALLBACK_PASS");
    }

    #[test]
    #[ignore = "requires verified developer binaries and public network access"]
    fn real_public_page_formats() {
        let root = PathBuf::from(std::env::var_os("DM_R4_TOOL_ROOT").unwrap());
        let address = std::env::var("DM_R4_TEST_PAGE").unwrap_or_else(|_| {
            "https://mdn.github.io/learning-area/html/multimedia-and-embedding/video-and-audio-content/extra-video-features.html".into()
        });
        let report = Analyzer::open(&root).unwrap().analyze(&address).unwrap();
        assert!(!report.formats.is_empty());
        assert!(report
            .formats
            .iter()
            .any(|format| format.container.as_deref() == Some("mp4")));
        println!("PUBLIC_FORMATS_PASS: {} format(s)", report.formats.len());
    }

    #[test]
    #[ignore = "requires verified developer binaries and public network access"]
    fn real_public_hls_formats() {
        let root = PathBuf::from(std::env::var_os("DM_R4_TOOL_ROOT").unwrap());
        let report = Analyzer::open(&root)
            .unwrap()
            .analyze("https://storage.googleapis.com/shaka-demo-assets/angel-one-hls/hls.m3u8")
            .unwrap();
        assert!(
            report
                .formats
                .iter()
                .filter(|format| format.height.is_some())
                .count()
                >= 2
        );
        println!("HLS_FORMATS_PASS: {} format(s)", report.formats.len());
    }
}
