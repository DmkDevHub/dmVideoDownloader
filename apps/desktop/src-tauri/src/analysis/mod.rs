pub(crate) mod process;
pub(crate) mod tools;

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

pub(crate) mod extract;
pub(crate) mod proxy;
use std::sync::atomic::{AtomicBool, Ordering};

static ACTIVE: AtomicBool = AtomicBool::new(false);
struct Active;
impl Active {
    fn acquire() -> Result<Self, String> {
        ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .map(|_| Self)
            .map_err(|_| "ANALYSIS_BUSY".into())
    }
}
impl Drop for Active {
    fn drop(&mut self) {
        ACTIVE.store(false, Ordering::Release);
    }
}

#[tauri::command]
pub async fn analyze_url(
    address: String,
    store: tauri::State<'_, std::sync::Mutex<crate::settings::Store>>,
) -> Result<extract::Report, String> {
    if proxy::target(&address).is_none() {
        return Err("URL_NOT_ALLOWED".into());
    }
    let root = store
        .lock()
        .map_err(|_| "SETTINGS_BUSY")?
        .root()
        .to_path_buf();
    let active = Active::acquire()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _active = active;
        extract::Analyzer::open(&root)?.analyze(&address)
    })
    .await
    .map_err(|_| "ANALYSIS_FAILED")?
    .map_err(str::to_owned)
}

#[tauri::command]
pub async fn analyze_candidate(
    id: String,
    store: tauri::State<'_, std::sync::Mutex<crate::settings::Store>>,
    bridge: tauri::State<'_, std::sync::Arc<crate::bridge::Bridge>>,
) -> Result<extract::Report, String> {
    let selected = bridge.candidate(&id)?;
    let root = store
        .lock()
        .map_err(|_| "SETTINGS_BUSY")?
        .root()
        .to_path_buf();
    let active = Active::acquire()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _active = active;
        let analyzer = extract::Analyzer::open(&root)?;
        extract::candidate(&selected.page_url, selected.media_url.as_deref(), |url| {
            analyzer.analyze(url)
        })
    })
    .await
    .map_err(|_| "ANALYSIS_FAILED")?
    .map_err(str::to_owned)
}
pub(crate) mod failure;
