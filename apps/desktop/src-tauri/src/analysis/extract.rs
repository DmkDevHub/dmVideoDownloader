use super::{
    process::{self, RunError},
    proxy,
    tools::{self, State, Tool},
};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

#[derive(Deserialize)]
struct RawReport {
    title: Option<String>,
    #[serde(default)]
    formats: Vec<RawFormat>,
}
#[derive(Deserialize)]
struct RawFormat {
    format_id: Option<String>,
    ext: Option<String>,
    height: Option<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Format {
    pub id: String,
    pub container: Option<String>,
    pub height: Option<u32>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub title: String,
    pub formats: Vec<Format>,
    pub source: Source,
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
    let formats: Vec<_> = raw
        .formats
        .into_iter()
        .filter_map(|raw| {
            Some(Format {
                id: format_tag(raw.format_id, 96)?,
                container: format_tag(raw.ext, 16),
                height: raw.height.filter(|height| (1..=32_768).contains(height)),
            })
        })
        .take(256)
        .collect();
    if formats.is_empty() {
        return Err("NO_FORMATS");
    }
    Ok(Report {
        title,
        formats,
        source: Source::Direct,
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
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(
                if error
                    .lines()
                    .any(|line| line.starts_with("ERROR: Unsupported URL:"))
                {
                    "UNSUPPORTED_URL"
                } else {
                    "ANALYSIS_FAILED"
                },
            );
        }
        report(&output.stdout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn projects_only_safe_metadata() {
        let result = report(br#"{"title":"Video","formats":[{"format_id":"18","ext":"mp4","height":720,"url":"https://example.org/?token=secret"}],"http_headers":{"Cookie":"secret"}}"#).unwrap();
        let json = serde_json::to_value(result).unwrap();
        assert_eq!(json["formats"][0]["height"], 720);
        assert!(!json.to_string().contains("secret"));
        assert!(matches!(report(br#"{"formats":[]}"#), Err("NO_FORMATS")));
    }

    #[test]
    fn candidate_only_falls_back_for_unsupported_pages_or_missing_formats() {
        let page = "https://example.org/page";
        let media = "https://example.org/video.mp4";
        for reason in [
            "UNSUPPORTED_URL",
            "NO_FORMATS",
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
}
