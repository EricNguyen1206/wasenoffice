//! Touch Bar integration for macOS MacBook Pro (Tauri command + native FFI).

use std::ffi::{c_char, c_int, c_void, CString};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

type TouchBarCallback = extern "C" fn(index: c_int, action: *const c_char);

#[cfg(target_os = "macos")]
extern "C" {
    fn wasen_touchbar_init(ns_window_ptr: *mut c_void, callback: TouchBarCallback);
    fn wasen_touchbar_update_tabs(titles: *const *const c_char, count: c_int, active_index: c_int);
}

static APP_HANDLE: Mutex<Option<AppHandle>> = Mutex::new(None);

extern "C" fn on_touchbar_action(index: c_int, action: *const c_char) {
    if action.is_null() {
        return;
    }
    let act = unsafe { std::ffi::CStr::from_ptr(action) }
        .to_string_lossy()
        .into_owned();
    if let Ok(guard) = APP_HANDLE.lock() {
        if let Some(app) = guard.as_ref() {
            let _ = app.emit(
                "touchbar-action",
                serde_json::json!({
                    "action": act,
                    "index": index
                }),
            );
        }
    }
}

pub fn init_touchbar(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    {
        if let Ok(mut guard) = APP_HANDLE.lock() {
            *guard = Some(window.app_handle().clone());
        }
        if let Ok(ns_win) = window.ns_window() {
            unsafe {
                wasen_touchbar_init(ns_win, on_touchbar_action);
            }
        }
    }
}

#[derive(serde::Deserialize)]
pub struct TouchBarTabPayload {
    pub titles: Vec<String>,
    pub active_index: i32,
}

#[tauri::command]
pub fn touchbar_update(payload: TouchBarTabPayload) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let cstrings: Vec<CString> = payload
            .titles
            .iter()
            .map(|t| CString::new(t.as_str()).unwrap_or_default())
            .collect();
        let ptrs: Vec<*const c_char> = cstrings.iter().map(|cs| cs.as_ptr()).collect();
        unsafe {
            wasen_touchbar_update_tabs(
                ptrs.as_ptr(),
                ptrs.len() as c_int,
                payload.active_index as c_int,
            );
        }
    }
    Ok(())
}
