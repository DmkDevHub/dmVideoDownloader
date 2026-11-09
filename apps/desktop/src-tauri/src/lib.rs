#[cfg(feature = "smoke-test")]
#[tauri::command]
fn smoke_ready(app: tauri::AppHandle, window: tauri::WebviewWindow, mounted: bool) {
    // A real native WebView window and the mounted Vue page must both exist.
    #[cfg(windows)]
    let native_window_exists = window.hwnd().is_ok();
    #[cfg(not(windows))]
    let native_window_exists = false;

    let passed = mounted
        && native_window_exists
        && window.label() == "main"
        && matches!(window.is_visible(), Ok(false));
    if passed {
        println!("SMOKE_PASS: native window, Vue mount, frontend-to-Rust IPC");
    }
    app.exit(if passed { 0 } else { 1 });
}

pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            #[cfg(feature = "smoke-test")]
            let root = std::path::PathBuf::from(
                std::env::var_os("DM_SMOKE_ROOT").ok_or("missing smoke root")?,
            );
            #[cfg(not(feature = "smoke-test"))]
            let root = std::env::current_exe()?
                .parent()
                .ok_or("missing executable directory")?
                .to_path_buf();
            let directory = default_download_directory(app.handle().clone())?;
            app.manage(std::sync::Mutex::new(settings::Store::new(
                root.clone(),
                directory,
            )));
            let bridge = std::sync::Arc::new(bridge::Bridge::new());
            let handle = app.handle().clone();
            let _ = bridge.start(&root, move |id| {
                let state = handle.state::<std::sync::Mutex<settings::Store>>();
                let store = state
                    .lock()
                    .map_err(|_| dm_video_protocol::Code::HostStartFailed)?;
                let saved = store
                    .current_settings()
                    .map_err(|_| dm_video_protocol::Code::ExtensionIdMismatch)?;
                if saved.browser_extension_id != id {
                    return Err(dm_video_protocol::Code::ExtensionIdMismatch);
                }
                #[cfg(feature = "smoke-test")]
                if matches!(
                    std::env::var("DM_SMOKE_PHASE").as_deref(),
                    Ok("bridge" | "bridge-analysis")
                ) {
                    return dm_video_protocol::manifest::Manifest::verify(store.root(), id)
                        .map(|()| saved.browser_auto_detect);
                }
                registration::Registry::chrome()
                    .status(store.root(), id)
                    .map(|()| saved.browser_auto_detect)
                    .map_err(browser::registration_error)
            });
            app.manage(bridge);

            #[cfg(feature = "smoke-test")]
            {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    let timeout =
                        if std::env::var("DM_SMOKE_PHASE").as_deref() == Ok("bridge-analysis") {
                            95
                        } else {
                            30
                        };
                    std::thread::sleep(std::time::Duration::from_secs(timeout));
                    handle.exit(1);
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            default_download_directory,
            pick_download_directory,
            validate_download_directory,
            load_settings,
            save_settings,
            recover_settings,
            analysis::tool_diagnostics,
            analysis::analyze_url,
            analysis::analyze_candidate,
            download::download_video,
            browser::browser_runtime,
            browser::browser_begin_test,
            browser::browser_connection_status,
            bridge::received_candidates,
            browser::browser_register,
            browser::browser_unregister,
            browser::open_extension_folder,
            browser::open_chrome_extensions,
            browser::open_download_folder
        ]);
    #[cfg(feature = "smoke-test")]
    let builder = builder.invoke_handler(tauri::generate_handler![
        default_download_directory,
        pick_download_directory,
        validate_download_directory,
        load_settings,
        save_settings,
        recover_settings,
        analysis::tool_diagnostics,
        analysis::analyze_url,
        analysis::analyze_candidate,
        browser::browser_runtime,
        browser::browser_begin_test,
        browser::browser_connection_status,
        bridge::received_candidates,
        browser::browser_register,
        browser::browser_unregister,
        browser::open_extension_folder,
        browser::open_chrome_extensions,
        browser::open_download_folder,
        bridge::smoke_bridge_received,
        smoke_phase,
        smoke_ready
    ]);

    builder
        .run(tauri::generate_context!())
        .expect("desktop runtime failed");
}
use tauri::Manager;

#[tauri::command]
fn default_download_directory(app: tauri::AppHandle) -> Result<String, String> {
    #[cfg(feature = "smoke-test")]
    if let Some(root) = std::env::var_os("DM_SMOKE_ROOT") {
        return Ok(std::path::PathBuf::from(root)
            .join("Downloads")
            .join("Videos")
            .to_string_lossy()
            .into_owned());
    }
    app.path()
        .download_dir()
        .map(|path| path.join("Videos").to_string_lossy().into_owned())
        .map_err(|_| "DOWNLOAD_DIRECTORY_UNAVAILABLE".into())
}
mod analysis;
mod bridge;
mod browser;
mod connection;
mod download;
mod folders;
mod registration;
mod settings;

#[cfg(feature = "smoke-test")]
#[tauri::command]
fn smoke_phase() -> String {
    std::env::var("DM_SMOKE_PHASE").unwrap_or_else(|_| "write".into())
}

#[tauri::command]
fn recover_settings(
    store: tauri::State<std::sync::Mutex<settings::Store>>,
    bridge: tauri::State<std::sync::Arc<bridge::Bridge>>,
) -> Result<settings::Snapshot, String> {
    let snapshot = store.lock().map_err(|_| "SETTINGS_BUSY")?.recover()?;
    bridge.invalidate();
    Ok(snapshot)
}

#[tauri::command]
fn load_settings(
    store: tauri::State<std::sync::Mutex<settings::Store>>,
) -> Result<settings::Snapshot, String> {
    store
        .lock()
        .map_err(|_| "SETTINGS_BUSY")?
        .load()
        .map_err(str::to_owned)
}

#[tauri::command]
fn save_settings(
    settings: settings::Settings,
    store: tauri::State<std::sync::Mutex<settings::Store>>,
    bridge: tauri::State<std::sync::Arc<bridge::Bridge>>,
) -> Result<settings::Settings, String> {
    let mut store = store.lock().map_err(|_| "SETTINGS_BUSY")?;
    let id_changed = store
        .current_settings()
        .is_ok_and(|saved| saved.browser_extension_id != settings.browser_extension_id);
    let saved = store.save(settings)?;
    if id_changed {
        bridge.invalidate();
    }
    Ok(saved)
}

#[tauri::command]
async fn pick_download_directory(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_parent(&window)
            .set_title("다운로드 저장 위치")
            .blocking_pick_folder()
    })
    .await
    .map_err(|_| "DIRECTORY_PICKER_FAILED")?;
    let Some(picked) = picked else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|_| "INVALID_DIRECTORY")?;
    folders::validate_directory(path.to_str().ok_or("INVALID_DIRECTORY")?)
        .map(|p| Some(p.to_string_lossy().into_owned()))
        .map_err(str::to_owned)
}

#[tauri::command]
async fn validate_download_directory(directory: String) -> Result<String, String> {
    folders::validate_directory(&directory)
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(str::to_owned)
}
