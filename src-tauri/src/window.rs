//! Window factory seam (T-046, Candidate 1).
//!
//! Centralizes construction of all WebviewWindow instances across the shell,
//! enforcing Rule 6b background sliver positioning, focused(false) invariants,
//! and script injections based on typed `AppConfig`.

use tauri::{WebviewUrl, WebviewWindowBuilder};

use crate::config::{AppConfig, BootMode};

/// Create the initial application boot window matching `config.boot_mode`.
pub fn create_boot_window(
    app: &tauri::AppHandle,
    config: &AppConfig,
    bridge_js: &str,
    delivery_js: &str,
) -> Result<(), tauri::Error> {
    let bg_pos = config.bg_sliver_position(app);

    match &config.boot_mode {
        BootMode::QuickView(_) => {
            let mut win =
                WebviewWindowBuilder::new(app, "main", WebviewUrl::App("viewer.html".into()))
                    .title("WasenOffice — Quick View")
                    .inner_size(1000.0, 700.0)
                    .visible(true)
                    .focused(false);
            if let Some((x, y)) = bg_pos {
                win = win.decorations(false).position(x, y);
            }
            if config.qv_hittest {
                win = win.initialization_script_for_all_frames(
                    "window.__WASEN_OFFICE_QV_HITTEST__ = true;",
                );
            }
            let _ = win.build()?;
            Ok(())
        }
        BootMode::NanoLight(_) => {
            let mut win =
                WebviewWindowBuilder::new(app, "main", WebviewUrl::App("editor.html".into()))
                    .title("WasenOffice — Nano Light")
                    .inner_size(1000.0, 700.0)
                    .visible(true)
                    .focused(false);
            if let Some((x, y)) = bg_pos {
                win = win.decorations(false).position(x, y);
            }
            if let Some(marker) = &config.nl_probe_marker {
                let script = format!(
                    "window.__WASEN_OFFICE_NL_PROBE__ = {};",
                    serde_json::json!(marker)
                );
                win = win.initialization_script_for_all_frames(&script);
            }
            let _ = win.build()?;
            Ok(())
        }
        BootMode::Normal => {
            let mut win = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title("WasenOffice")
                .inner_size(1280.0, 800.0)
                .min_inner_size(940.0, 560.0)
                .visible(true)
                .focused(false);
            if let Some((x, y)) = bg_pos {
                win = win.decorations(false).position(x, y);
            }

            for script in config.normal_probe_scripts() {
                if !script.is_empty() {
                    win = win.initialization_script_for_all_frames(&script);
                }
            }
            if !bridge_js.is_empty() {
                win = win.initialization_script_for_all_frames(bridge_js);
            }
            if !delivery_js.is_empty() {
                win = win.initialization_script_for_all_frames(delivery_js);
            }

            let win = win.build()?;
            #[cfg(debug_assertions)]
            if config.devtools {
                win.open_devtools();
            }
            let _ = win;
            Ok(())
        }
    }
}

/// Shared surface-window builder for dedicated UI-opened windows (viewer, editor).
pub(crate) fn open_surface_window(
    app: &tauri::AppHandle,
    label: &str,
    html: &str,
    title: String,
    tag: &str,
) -> Result<(), String> {
    use std::sync::Arc;
    use tauri::Manager;

    let mut b = WebviewWindowBuilder::new(app, label, WebviewUrl::App(html.into()))
        .title(title)
        .inner_size(1000.0, 700.0)
        .visible(true)
        .focused(false);
    if let Some(config) = app.try_state::<Arc<AppConfig>>() {
        if let Some((x, y)) = config.bg_sliver_position(app) {
            b = b.decorations(false).position(x, y);
        }
    }
    b.build().map_err(|e| format!("{tag}: open window: {e}"))?;
    Ok(())
}

/// Thread-safe window-keyed document path handoff (T-063, Candidate 3).
///
/// Eliminates the single-slot race where rapid opens overwrite unconsumed paths.
/// Keyed by window label, matching the T-059 FileRegistry owner model.
#[derive(Default)]
pub struct SurfaceHandoff {
    entries: std::sync::Mutex<std::collections::HashMap<String, String>>,
}

impl SurfaceHandoff {
    pub fn stash(&self, label: &str, path: String) {
        self.entries
            .lock()
            .expect("surface handoff mutex poisoned")
            .insert(label.to_string(), path);
    }

    pub fn take(&self, label: &str) -> Option<String> {
        self.entries
            .lock()
            .expect("surface handoff mutex poisoned")
            .remove(label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_surface_handoff_keyed_multiple_windows() {
        let handoff = SurfaceHandoff::default();
        handoff.stash("quickview-1", "/docs/a.docx".into());
        handoff.stash("quickview-2", "/docs/b.docx".into());

        // Each window takes only its own path, order independent
        assert_eq!(handoff.take("quickview-2"), Some("/docs/b.docx".into()));
        assert_eq!(handoff.take("quickview-1"), Some("/docs/a.docx".into()));
    }

    #[test]
    fn test_surface_handoff_take_once() {
        let handoff = SurfaceHandoff::default();
        handoff.stash("nanolight-1", "/docs/edit.docx".into());

        assert_eq!(handoff.take("nanolight-1"), Some("/docs/edit.docx".into()));
        assert_eq!(handoff.take("nanolight-1"), None);
        assert_eq!(handoff.take("unknown"), None);
    }
}
