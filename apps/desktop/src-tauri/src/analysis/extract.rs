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
    Ok(Report { title, formats })
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
            return Err("ANALYSIS_FAILED");
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
