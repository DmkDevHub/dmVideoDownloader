mod process;
mod tools;

#[tauri::command]
pub async fn tool_diagnostics(
    store: tauri::State<'_, std::sync::Mutex<crate::settings::Store>>,
) -> Result<tools::Diagnostics, String> {
    let root = store
        .lock()
        .map_err(|_| "SETTINGS_BUSY")?
        .root()
        .to_path_buf();
    tauri::async_runtime::spawn_blocking(move || tools::inspect(&root))
        .await
        .map_err(|_| "TOOL_DIAGNOSTICS_FAILED".into())
}
