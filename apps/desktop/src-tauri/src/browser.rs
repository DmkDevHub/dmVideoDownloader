use crate::{folders, settings::Store};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInfo {
    extension_path: String,
    extension_resources_available: bool,
    host_available: bool,
    chrome_available: bool,
    connection_actions_available: bool,
}

fn chrome_path() -> Option<PathBuf> {
    ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"]
        .iter()
        .filter_map(std::env::var_os)
        .map(|root| PathBuf::from(root).join("Google/Chrome/Application/chrome.exe"))
        .find(|path| path.is_file())
}

fn inspect(root: &Path) -> RuntimeInfo {
    let extension = root.join("extension");
    RuntimeInfo {
        extension_path: extension.to_string_lossy().into_owned(),
        extension_resources_available: [
            "manifest.json",
            "background.js",
            "content.js",
            "popup.html",
            "popup.js",
        ]
        .iter()
        .all(|name| extension.join(name).is_file()),
        host_available: root.join("VideoNativeHost.exe").is_file(),
        chrome_available: chrome_path().is_some(),
        // Registry and round-trip implementation belong to R3, not this UI stage.
        connection_actions_available: false,
    }
}

#[tauri::command]
pub fn browser_runtime(store: tauri::State<Mutex<Store>>) -> Result<RuntimeInfo, String> {
    Ok(inspect(store.lock().map_err(|_| "SETTINGS_BUSY")?.root()))
}

fn open_folder(path: &Path) -> Result<(), String> {
    folders::check_path(path).map_err(str::to_owned)?;
    if !path.is_dir() {
        return Err("DIRECTORY_NOT_FOUND".into());
    }
    let explorer = PathBuf::from(std::env::var_os("SystemRoot").ok_or("EXPLORER_UNAVAILABLE")?)
        .join("explorer.exe");
    Command::new(explorer)
        .arg(path)
        .spawn()
        .map_err(|_| "FOLDER_OPEN_FAILED")?;
    Ok(())
}

#[tauri::command]
pub fn open_extension_folder(store: tauri::State<Mutex<Store>>) -> Result<(), String> {
    let root = store
        .lock()
        .map_err(|_| "SETTINGS_BUSY")?
        .root()
        .to_path_buf();
    if !inspect(&root).extension_resources_available {
        return Err("EXTENSION_RESOURCES_MISSING".into());
    }
    open_folder(&root.join("extension"))
}

#[tauri::command]
pub fn open_download_folder(store: tauri::State<Mutex<Store>>) -> Result<(), String> {
    let path = store
        .lock()
        .map_err(|_| "SETTINGS_BUSY")?
        .saved_directory()
        .map_err(str::to_owned)?;
    open_folder(&path)
}

#[tauri::command]
pub fn open_chrome_extensions() -> Result<(), String> {
    Command::new(chrome_path().ok_or("CHROME_NOT_FOUND")?)
        .arg("chrome://extensions")
        .spawn()
        .map_err(|_| "CHROME_OPEN_FAILED")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resources_and_connection_success_are_distinct() {
        let root = tempfile::tempdir().unwrap();
        let empty = inspect(root.path());
        assert!(!empty.extension_resources_available && !empty.host_available);
        std::fs::create_dir(root.path().join("extension")).unwrap();
        for name in [
            "manifest.json",
            "background.js",
            "content.js",
            "popup.html",
            "popup.js",
        ] {
            std::fs::write(root.path().join("extension").join(name), b"fixture").unwrap();
        }
        std::fs::write(root.path().join("VideoNativeHost.exe"), b"fixture").unwrap();
        let resources = inspect(root.path());
        assert!(resources.extension_resources_available && resources.host_available);
        assert!(!resources.connection_actions_available);
    }
}
