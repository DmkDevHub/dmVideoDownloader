use tauri::AppHandle;

#[cfg(feature = "smoke-test")]
use std::sync::atomic::{AtomicUsize, Ordering};
#[cfg(feature = "smoke-test")]
static NOTIFICATIONS: AtomicUsize = AtomicUsize::new(0);

#[cfg(feature = "smoke-test")]
#[tauri::command]
pub fn smoke_notification_count() -> usize {
    NOTIFICATIONS.load(Ordering::Acquire)
}

pub(super) fn completed(app: &AppHandle, id: u32, filename: String) {
    #[cfg(feature = "smoke-test")]
    if std::env::var("DM_R5_TEST_WINDOWS_NOTIFICATION").as_deref() != Ok("1") {
        NOTIFICATIONS.fetch_add(1, Ordering::Release);
        return;
    }
    use std::{ptr, time::Duration};
    use tauri::Manager;
    use windows_sys::Win32::UI::{
        Shell::{
            Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_TIP, NIIF_INFO, NIIF_RESPECT_QUIET_TIME,
            NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
        },
        WindowsAndMessaging::{LoadIconW, IDI_APPLICATION},
    };

    fn copy_text(target: &mut [u16], value: &str) {
        let capacity = target.len() - 1;
        let mut length = 0;
        for ch in value.chars() {
            let mut buffer = [0; 2];
            let part = ch.encode_utf16(&mut buffer);
            if length + part.len() > capacity {
                break;
            }
            target[length..length + part.len()].copy_from_slice(part);
            length += part.len();
        }
    }

    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Some(window) = handle.get_webview_window("main") else {
            return;
        };
        let Ok(hwnd) = window.hwnd() else {
            return;
        };
        let mut icon = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd.0,
            uID: id,
            uFlags: NIF_ICON | NIF_TIP,
            hIcon: unsafe { LoadIconW(ptr::null_mut(), IDI_APPLICATION) },
            ..Default::default()
        };
        copy_text(&mut icon.szTip, "영상 다운로더");
        if unsafe { Shell_NotifyIconW(NIM_ADD, &icon) } == 0 {
            return;
        }
        icon.uFlags = NIF_INFO;
        icon.dwInfoFlags = NIIF_INFO | NIIF_RESPECT_QUIET_TIME;
        copy_text(&mut icon.szInfoTitle, "영상 다운로드 완료");
        copy_text(&mut icon.szInfo, &filename);
        if unsafe { Shell_NotifyIconW(NIM_MODIFY, &icon) } == 0 {
            unsafe { Shell_NotifyIconW(NIM_DELETE, &icon) };
            return;
        }
        #[cfg(feature = "smoke-test")]
        NOTIFICATIONS.fetch_add(1, Ordering::Release);
        let hwnd = hwnd.0 as usize;
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(25));
            let _ = handle.run_on_main_thread(move || {
                let icon = NOTIFYICONDATAW {
                    cbSize: size_of::<NOTIFYICONDATAW>() as u32,
                    hWnd: hwnd as _,
                    uID: id,
                    ..Default::default()
                };
                unsafe { Shell_NotifyIconW(NIM_DELETE, &icon) };
            });
        });
    });
}
