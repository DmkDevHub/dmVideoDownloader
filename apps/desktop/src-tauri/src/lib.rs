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
        .invoke_handler(tauri::generate_handler![
            default_download_directory,
            pick_download_directory,
            validate_download_directory
        ]);
    #[cfg(feature = "smoke-test")]
    let builder = builder
        .invoke_handler(tauri::generate_handler![
            default_download_directory,
            pick_download_directory,
            validate_download_directory,
            smoke_ready
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(30));
                handle.exit(1);
            });
            Ok(())
        });

    builder
        .run(tauri::generate_context!())
        .expect("desktop runtime failed");
}
use tauri::Manager;

#[tauri::command]
fn default_download_directory(app: tauri::AppHandle) -> Result<String, String> {
    app.path()
        .download_dir()
        .map(|path| path.join("Videos").to_string_lossy().into_owned())
        .map_err(|_| "DOWNLOAD_DIRECTORY_UNAVAILABLE".into())
}
mod folders;

#[tauri::command]
async fn pick_download_directory(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
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
