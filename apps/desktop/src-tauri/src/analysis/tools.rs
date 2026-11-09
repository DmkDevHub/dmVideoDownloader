use super::process::{self, RunError};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

#[derive(Clone, Copy)]
pub enum Tool {
    YtDlp,
    Ffmpeg,
}
impl Tool {
    fn filename(self) -> &'static str {
        match self {
            Self::YtDlp => "yt-dlp.exe",
            Self::Ffmpeg => "ffmpeg.exe",
        }
    }
    fn version_args(self) -> &'static [&'static str] {
        match self {
            Self::YtDlp => &["--ignore-config", "--version"],
            Self::Ffmpeg => &["-version"],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum State {
    Ready,
    Missing,
    InvalidPath,
    Failed,
    Timeout,
    Unsupported,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub state: State,
    pub version: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    pub yt_dlp: Status,
    pub ffmpeg: Status,
}

pub fn executable(root: &Path, tool: Tool) -> Result<PathBuf, State> {
    let path = root.join("tools").join(tool.filename());
    crate::folders::check_path(&path).map_err(|_| State::InvalidPath)?;
    match std::fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() => Ok(path),
        Ok(_) => Err(State::InvalidPath),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(State::Missing),
        Err(_) => Err(State::Failed),
    }
}
fn version(tool: Tool, stdout: &[u8]) -> Option<(String, bool)> {
    let line = std::str::from_utf8(stdout).ok()?.lines().next()?.trim();
    let value = match tool {
        Tool::YtDlp => line,
        Tool::Ffmpeg => line
            .strip_prefix("ffmpeg version ")?
            .split_whitespace()
            .next()?,
    };
    let version = value.split('-').next()?;
    if version.len() > 32 || !version.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return None;
    }
    let parts: Vec<u32> = version
        .split('.')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    let supported = match tool {
        Tool::YtDlp => match parts.as_slice() {
            [year, month, day, ..] if (1..=12).contains(month) && (1..=31).contains(day) => {
                (*year, *month, *day) >= (2025, 11, 12)
            }
            _ => return None,
        },
        Tool::Ffmpeg if parts.len() >= 2 => parts[0] >= 6,
        Tool::Ffmpeg => return None,
    };
    Some((version.into(), supported))
}
pub fn probe(root: &Path, tool: Tool) -> Status {
    let failed = |state| Status {
        state,
        version: None,
    };
    let path = match executable(root, tool) {
        Ok(path) => path,
        Err(state) => return failed(state),
    };
    let output = process::run(
        Command::new(path)
            .current_dir(root)
            .args(tool.version_args()),
        Duration::from_secs(8),
        16_384,
        16_384,
    );
    match output {
        Ok(output) if output.success => {
            let _ = output.stderr;
            match version(tool, &output.stdout) {
                Some((version, supported)) => Status {
                    state: if supported {
                        State::Ready
                    } else {
                        State::Unsupported
                    },
                    version: Some(version),
                },
                None => failed(State::Unsupported),
            }
        }
        Err(RunError::Timeout) => failed(State::Timeout),
        _ => failed(State::Failed),
    }
}
pub fn inspect(root: &Path) -> Diagnostics {
    Diagnostics {
        yt_dlp: probe(root, Tool::YtDlp),
        ffmpeg: probe(root, Tool::Ffmpeg),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recognizes_supported_versions_and_rejects_old_or_unexpected_output() {
        assert_eq!(
            version(Tool::YtDlp, b"2025.11.12\n"),
            Some(("2025.11.12".into(), true))
        );
        assert_eq!(
            version(Tool::YtDlp, b"2024.12.01\n"),
            Some(("2024.12.01".into(), false))
        );
        assert_eq!(
            version(Tool::Ffmpeg, b"ffmpeg version 8.1.1-build\n"),
            Some(("8.1.1".into(), true))
        );
        assert_eq!(
            version(Tool::Ffmpeg, b"ffmpeg version 5.1.3\n"),
            Some(("5.1.3".into(), false))
        );
        assert!(version(Tool::YtDlp, b"unexpected private path").is_none());
    }
    #[test]
    fn missing_and_broken_executables_have_different_status() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(probe(root.path(), Tool::YtDlp).state, State::Missing);
        std::fs::create_dir(root.path().join("tools")).unwrap();
        std::fs::write(
            root.path().join("tools").join("yt-dlp.exe"),
            b"invalid executable",
        )
        .unwrap();
        assert_eq!(probe(root.path(), Tool::YtDlp).state, State::Failed);
    }
    #[test]
    #[ignore = "requires the developer's verified portable test binaries"]
    fn real_tool_versions() {
        let root = PathBuf::from(std::env::var_os("DM_R4_TOOL_ROOT").unwrap());
        let tools = inspect(&root);
        assert_eq!(tools.yt_dlp.state, State::Ready);
        assert_eq!(tools.ffmpeg.state, State::Ready);
        println!(
            "TOOL_VERSIONS_PASS: yt-dlp {} / FFmpeg {}",
            tools.yt_dlp.version.unwrap(),
            tools.ffmpeg.version.unwrap()
        );
    }
}
