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
    let builder = tauri::Builder::default();
    #[cfg(feature = "smoke-test")]
    let builder = builder
        .invoke_handler(tauri::generate_handler![smoke_ready])
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
