use crate::{bridge::Bridge, connection::Status, folders, registration::Registry, settings::Store};
use dm_video_protocol::Code;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInfo {
    extension_path: String,
    extension_resources_available: bool,
    host_available: bool,
    chrome_available: bool,
    connection_actions_available: bool,
    registration_code: String,
    registered_path: Option<String>,
    registry32_path: Option<String>,
    registry64_path: Option<String>,
    configured_id: String,
    manifest_path: String,
}

fn chrome_path() -> Option<PathBuf> {
    ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"]
        .iter()
        .filter_map(std::env::var_os)
        .map(|root| PathBuf::from(root).join("Google/Chrome/Application/chrome.exe"))
        .find(|path| path.is_file())
}

fn inspect(root: &Path, id: &str) -> RuntimeInfo {
    let extension = root.join("extension");
    let registry = Registry::chrome();
    let [registry32_path, registry64_path] = registry
        .views()
        .unwrap_or_default()
        .map(|entry| entry.map(|path| path.to_string_lossy().into_owned()));
    RuntimeInfo {
        configured_id: id.into(),
        manifest_path: root
            .join(dm_video_protocol::MANIFEST)
            .to_string_lossy()
            .into_owned(),
        extension_path: extension.to_string_lossy().into_owned(),
        extension_resources_available: [
            "manifest.json",
            "background.js",
            "content.js",
            "popup.html",
            "popup.js",
            "popup.css",
        ]
        .iter()
        .all(|name| extension.join(name).is_file()),
        host_available: root.join("VideoNativeHost.exe").is_file(),
        chrome_available: chrome_path().is_some(),
        connection_actions_available: true,
        registration_code: registry
            .status(root, id)
            .err()
            .unwrap_or("REGISTERED")
            .into(),
        registered_path: registry32_path.clone().or_else(|| registry64_path.clone()),
        registry32_path,
        registry64_path,
    }
}

#[tauri::command]
pub fn browser_runtime(store: tauri::State<Mutex<Store>>) -> Result<RuntimeInfo, String> {
    let store = store.lock().map_err(|_| "SETTINGS_BUSY")?;
    let id = store
        .current_settings()
        .map(|settings| settings.browser_extension_id)
        .unwrap_or_default();
    Ok(inspect(store.root(), &id))
}

#[tauri::command]
pub fn browser_register(
    extension_id: String,
    repair: Option<bool>,
    store: tauri::State<Mutex<Store>>,
    bridge: tauri::State<Arc<Bridge>>,
) -> Result<(), String> {
    let mut store = store.lock().map_err(|_| "SETTINGS_BUSY")?;
    bridge.invalidate();
    Registry::chrome()
        .register(&mut store, &extension_id, repair.unwrap_or(false))
        .map_err(str::to_owned)
}

#[tauri::command]
pub fn browser_unregister(
    store: tauri::State<Mutex<Store>>,
    bridge: tauri::State<Arc<Bridge>>,
) -> Result<(), String> {
    let store = store.lock().map_err(|_| "SETTINGS_BUSY")?;
    bridge.invalidate();
    Registry::chrome()
        .remove(store.root())
        .map_err(str::to_owned)
}

#[tauri::command]
pub fn browser_begin_test(
    store: tauri::State<Mutex<Store>>,
    bridge: tauri::State<Arc<Bridge>>,
) -> Result<Status, String> {
    let store = store.lock().map_err(|_| "SETTINGS_BUSY")?;
    let settings = store.current_settings()?;
    Registry::chrome().status(store.root(), &settings.browser_extension_id)?;
    bridge.begin_test().map_err(str::to_owned)
}

#[tauri::command]
pub fn browser_connection_status(
    store: tauri::State<Mutex<Store>>,
    bridge: tauri::State<Arc<Bridge>>,
) -> Result<Status, String> {
    let status = bridge.status()?;
    if !matches!(status.phase, "waiting" | "connected") {
        return Ok(status);
    }
    let store = store.lock().map_err(|_| "SETTINGS_BUSY")?;
    let result = store.current_settings().and_then(|settings| {
        Registry::chrome().status(store.root(), &settings.browser_extension_id)
    });
    if let Err(code) = result {
        bridge.registration_lost(registration_error(code));
    }
    bridge.status().map_err(str::to_owned)
}

pub fn registration_error(error: &str) -> Code {
    match error {
        "EXTENSION_ID_MISMATCH" | "INVALID_EXTENSION_ID" => Code::ExtensionIdMismatch,
        "HOST_PATH_MISMATCH" => Code::HostPathMismatch,
        "HOST_NOT_REGISTERED" | "MANIFEST_MISSING" | "HOST_MISSING" => Code::HostNotRegistered,
        _ => Code::RegistrationFailed,
    }
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

pub(crate) fn select_file(path: &Path) -> Result<(), &'static str> {
    use std::{os::windows::ffi::OsStrExt, ptr};
    use windows_sys::Win32::{
        Foundation::RPC_E_CHANGED_MODE,
        System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED},
        UI::Shell::{ILCreateFromPathW, ILFree, SHOpenFolderAndSelectItems},
    };
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    // The shell calls use the current thread's COM apartment and a null-terminated path.
    unsafe {
        let initialized = CoInitializeEx(ptr::null(), COINIT_APARTMENTTHREADED as u32);
        if initialized < 0 && initialized != RPC_E_CHANGED_MODE {
            return Err("DOWNLOAD_REVEAL_FAILED");
        }
        let item = ILCreateFromPathW(wide.as_ptr());
        let selected = if item.is_null() {
            false
        } else {
            let result = SHOpenFolderAndSelectItems(item, 0, ptr::null(), 0);
            ILFree(item);
            result >= 0
        };
        if initialized >= 0 {
            CoUninitialize();
        }
        selected.then_some(()).ok_or("DOWNLOAD_REVEAL_FAILED")
    }
}

#[tauri::command]
pub fn open_extension_folder(store: tauri::State<Mutex<Store>>) -> Result<(), String> {
    let root = store
        .lock()
        .map_err(|_| "SETTINGS_BUSY")?
        .root()
        .to_path_buf();
    if !inspect(&root, "").extension_resources_available {
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
        let empty = inspect(root.path(), "");
        assert!(!empty.extension_resources_available && !empty.host_available);
        std::fs::create_dir(root.path().join("extension")).unwrap();
        for name in [
            "manifest.json",
            "background.js",
            "content.js",
            "popup.html",
            "popup.js",
            "popup.css",
        ] {
            std::fs::write(root.path().join("extension").join(name), b"fixture").unwrap();
        }
        std::fs::write(root.path().join("VideoNativeHost.exe"), b"fixture").unwrap();
        let resources = inspect(root.path(), "");
        assert!(resources.extension_resources_available && resources.host_available);
        assert_ne!(resources.registration_code, "REGISTERED");
    }
}
