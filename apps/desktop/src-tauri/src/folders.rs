use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf, Prefix};

pub fn check_path(path: &Path) -> Result<(), &'static str> {
    let text = path.to_str().ok_or("INVALID_DIRECTORY")?;
    if text.len() > 240 || text.contains('/') {
        return Err("INVALID_DIRECTORY");
    }
    let mut parts = path.components();
    if !matches!(parts.next(), Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::Disk(_)))
        || !matches!(parts.next(), Some(Component::RootDir))
    {
        return Err("ABSOLUTE_DIRECTORY_REQUIRED");
    }
    // Reject traversal and ambiguous Windows names before touching the filesystem.
    let rest = text.get(3..).ok_or("INVALID_DIRECTORY")?;
    if rest.is_empty() {
        return Err("UNSAFE_DIRECTORY");
    }
    for part in rest.trim_end_matches('\\').split('\\') {
        let upper = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        let reserved = matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || (upper.len() == 4
                && (upper.starts_with("COM") || upper.starts_with("LPT"))
                && matches!(upper.as_bytes()[3], b'1'..=b'9'));
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.ends_with(['.', ' '])
            || part
                .chars()
                .any(|c| c.is_control() || "<>:\"|?*".contains(c))
            || reserved
        {
            return Err("INVALID_DIRECTORY");
        }
    }
    let lower = text.trim_end_matches('\\').to_lowercase();
    for variable in [
        "SystemRoot",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramData",
    ] {
        if let Some(root) = std::env::var_os(variable) {
            let protected = root.to_string_lossy().to_lowercase();
            if lower == protected || lower.starts_with(&(protected + "\\")) {
                return Err("UNSAFE_DIRECTORY");
            }
        }
    }
    if rest.split('\\').any(|part| {
        part.eq_ignore_ascii_case("$Recycle.Bin")
            || part.eq_ignore_ascii_case("System Volume Information")
    }) {
        return Err("UNSAFE_DIRECTORY");
    }
    for ancestor in path.ancestors() {
        if let Ok(metadata) = fs::symlink_metadata(ancestor) {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err("UNSAFE_DIRECTORY");
            }
        }
    }
    Ok(())
}

pub fn validate_directory(text: &str) -> Result<PathBuf, &'static str> {
    let path = Path::new(text);
    check_path(path)?;
    fs::create_dir_all(path).map_err(|_| "DIRECTORY_NOT_WRITABLE")?;
    if !path.is_dir() {
        return Err("NOT_A_DIRECTORY");
    }
    check_path(path)?;
    let mut probe = tempfile::Builder::new()
        .prefix(".dm-write-check-")
        .tempfile_in(path)
        .map_err(|_| "DIRECTORY_NOT_WRITABLE")?;
    probe
        .write_all(b"write-check")
        .and_then(|_| probe.as_file().sync_all())
        .map_err(|_| "DIRECTORY_NOT_WRITABLE")?;
    probe.close().map_err(|_| "DIRECTORY_NOT_WRITABLE")?;
    Ok(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsafe_windows_paths_before_io() {
        for path in [
            "relative",
            "C:relative",
            "C:\\",
            "\\\\server\\share",
            "\\\\?\\C:\\Videos",
            "C:\\Videos\\..\\Windows",
            "C:\\Videos\\CON.txt",
            "C:\\Videos\\file:stream",
            "C:\\Videos ",
        ] {
            assert!(check_path(Path::new(path)).is_err(), "{path}");
        }
        let windows = std::env::var("SystemRoot").unwrap();
        assert_eq!(check_path(Path::new(&windows)), Err("UNSAFE_DIRECTORY"));
    }

    #[test]
    fn creates_unicode_directory_and_removes_probe_without_touching_files() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("한글 folder");
        let result = validate_directory(folder.to_str().unwrap()).unwrap();
        assert_eq!(result, folder);
        assert_eq!(fs::read_dir(folder).unwrap().count(), 0);
        let file = root.path().join("existing.txt");
        fs::write(&file, b"preserve").unwrap();
        assert!(validate_directory(file.to_str().unwrap()).is_err());
        assert_eq!(fs::read(file).unwrap(), b"preserve");
    }
}
