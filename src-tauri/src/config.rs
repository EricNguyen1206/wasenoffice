//! Application launch configuration & environment options (T-046).
//!
//! Gathers environment-driven knobs for app launch, probes, and boot mode
//! into a single typed struct with deterministic parsing. Provides a pure
//! lookup-based constructor for thread-safe unit testing without mutating
//! process environment variables.

/// The entry mode the boot window opens into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootMode {
    /// Standard application flow: start page or default document editor.
    Normal,
    /// Quick view surface (V01, DD V01 §3.1) triggered by `WASEN_OFFICE_QUICK_VIEW`.
    QuickView(String),
    /// Nano Light editor surface (L01, DD L01 §3) triggered by `WASEN_OFFICE_NANO_LIGHT`.
    NanoLight(String),
}

/// Typed configuration snapshot parsed at startup.
pub struct AppConfig {
    /// Rule 6b background execution mode (`WASEN_OFFICE_BACKGROUND=1`).
    pub background: bool,
    /// Startup document path (`WASEN_OFFICE_OPEN`), consumed once on load.
    startup_document: std::sync::Mutex<Option<String>>,
    /// Devtools toggle in debug builds (`WASEN_OFFICE_DEVTOOLS=1`).
    #[allow(dead_code)]
    pub devtools: bool,
    /// Boot mode and entry window flavor.
    pub boot_mode: BootMode,
    /// Whether frontend diagnostic probes are enabled (`WASEN_OFFICE_PROBE`).
    pub probe_enabled: bool,
    /// Marker text for save probe automation (`WASEN_OFFICE_SAVE_PROBE`).
    pub save_probe_text: Option<String>,
    /// Document path for start page UX probe (`WASEN_OFFICE_UX_PROBE`).
    pub ux_probe_path: Option<String>,
    /// Marker text for Nano Light edit probe (`WASEN_OFFICE_NL_PROBE`).
    pub nl_probe_marker: Option<String>,
    /// Quick view layout hit-test probe (`WASEN_OFFICE_QV_HITTEST`).
    pub qv_hittest: bool,
    /// Start page probe driver (`WASEN_OFFICE_STARTPAGE_PROBE`), e.g.
    /// `recent:1` — the gate 17 wiring driver (A01 T-A38).
    pub startpage_probe: Option<String>,
    /// Recents store override (`WASEN_OFFICE_RECENTS_FILE`) — tests/gates so
    /// verify never touches the user's real recents (A01 T-A38).
    pub recents_file: Option<String>,
    /// Save-As panel stub (`WASEN_OFFICE_SAVEAS_STUB`) — path used instead of
    /// the native NSSavePanel, which is not scriptable (A01 T-A39).
    pub saveas_stub: Option<String>,
    /// Untitled-dir override (`WASEN_OFFICE_UNTITLED_DIR`) — tests/gates so
    /// Create-new never writes into the user's real untitled dir (A01 T-A39).
    pub untitled_dir: Option<String>,
    /// Compact editor chrome (T-A40, pivoted 18.10 — DD U01 §8): DEFAULT OFF —
    /// the engine draws its own chrome per doctype (PDF: Forms/Redact/Plugins).
    /// `WASEN_OFFICE_COMPACT_CHROME=1` opts back in (our one-row header +
    /// engine tabs strip hidden). Code stays, path is dormant by default.
    pub compact_chrome: bool,
    /// Tab reachability spike probe (`WASEN_OFFICE_TAB_PROBE`) — T-A40 DD
    /// criterion 1: logs `Common`/NotificationCenter presence per frame.
    pub tab_probe: bool,
    /// Keymap spike probe (`WASEN_OFFICE_KM_PROBE`) — DD K02 S1: logs
    /// `window.editor` reachability + engine button id presence (app frame).
    pub km_probe: bool,
    /// Keymap kill switch (`WASEN_OFFICE_NO_KEYMAP=1`) — bisect/debug: shim
    /// skips the native shortcut apply entirely (DD K02 §4 fail-soft family).
    pub no_keymap: bool,
    /// PDF edit-text click probe (`WASEN_OFFICE_PDF_PROBE`) — synthetic click
    /// on the pdfeditor Edit Text button after boot (repro/diagnostics).
    pub pdf_probe: bool,
    /// Dev-only window frame (`WASEN_OFFICE_WIN_POS=x,y[,w,h]`) — on-screen
    /// placement for real-input automation runs (AppleScript drive/pixels).
    /// Pure debug knob: never set by the product.
    pub win_frame: Option<(f64, f64, f64, f64)>,
    /// Web dev mode (T-068, DD W01): `WASEN_OFFICE_WEB_BRIDGE=1` lets the Vite
    /// frontend in a plain browser reach the command layer over the loopback
    /// file server (`POST /invoke/<cmd>`). DEFAULT OFF — release never sets it.
    pub web_bridge: bool,
}

/// Parse `WASEN_OFFICE_WIN_POS=x,y[,w,h]` — 2 or 4 comma-separated numbers.
fn win_frame_lookup(lookup: impl Fn(&str) -> Option<String>) -> Option<(f64, f64, f64, f64)> {
    let v = lookup("WASEN_OFFICE_WIN_POS").filter(|v| !v.trim().is_empty())?;
    let nums: Vec<f64> = v
        .split(',')
        .filter_map(|p| p.trim().parse::<f64>().ok())
        .collect();
    match nums.as_slice() {
        [x, y] => Some((*x, *y, 1280.0, 800.0)),
        [x, y, w, h] => Some((*x, *y, *w, *h)),
        _ => None,
    }
}

impl AppConfig {
    /// Parse configuration from the live process environment.
    pub fn from_env() -> Self {
        let cfg = Self::from_lookup(|k| std::env::var(k).ok());
        if cfg
            .startup_document
            .lock()
            .map(|d| d.is_none())
            .unwrap_or(false)
        {
            for arg in std::env::args().skip(1) {
                if !arg.starts_with('-') && std::path::Path::new(&arg).is_file() {
                    if let Ok(mut lock) = cfg.startup_document.lock() {
                        *lock = Some(arg);
                    }
                    break;
                }
            }
        }
        cfg
    }

    /// Pure parser from any lookup function (thread-safe for unit tests).
    pub fn from_lookup<F>(lookup: F) -> Self
    where
        F: Fn(&str) -> Option<String>,
    {
        let non_empty = |k: &str| lookup(k).filter(|v| !v.trim().is_empty());
        let background = matches!(
            lookup("WASEN_OFFICE_BACKGROUND").as_deref(),
            Some(v) if !v.is_empty() && v != "0"
        );

        let startup_document = std::sync::Mutex::new(non_empty("WASEN_OFFICE_OPEN"));
        let devtools = matches!(lookup("WASEN_OFFICE_DEVTOOLS").as_deref(), Some("1"));

        let boot_mode = if let Some(path) = non_empty("WASEN_OFFICE_QUICK_VIEW") {
            BootMode::QuickView(path)
        } else if let Some(path) = non_empty("WASEN_OFFICE_NANO_LIGHT") {
            BootMode::NanoLight(path)
        } else {
            BootMode::Normal
        };

        let probe_enabled = match lookup("WASEN_OFFICE_PROBE").as_deref() {
            Some("0") => false,
            Some("1") => true,
            _ => cfg!(debug_assertions),
        };

        let save_probe_text = non_empty("WASEN_OFFICE_SAVE_PROBE");
        let ux_probe_path = non_empty("WASEN_OFFICE_UX_PROBE");
        let nl_probe_marker = non_empty("WASEN_OFFICE_NL_PROBE");
        let qv_hittest = lookup("WASEN_OFFICE_QV_HITTEST").is_some_and(|v| !v.is_empty());
        let startpage_probe = non_empty("WASEN_OFFICE_STARTPAGE_PROBE");
        let recents_file = non_empty("WASEN_OFFICE_RECENTS_FILE");
        let saveas_stub = non_empty("WASEN_OFFICE_SAVEAS_STUB");
        let untitled_dir = non_empty("WASEN_OFFICE_UNTITLED_DIR");
        let compact_chrome = matches!(lookup("WASEN_OFFICE_COMPACT_CHROME").as_deref(), Some("1"));
        let tab_probe = lookup("WASEN_OFFICE_TAB_PROBE").is_some_and(|v| !v.is_empty());
        let km_probe = lookup("WASEN_OFFICE_KM_PROBE").is_some_and(|v| !v.is_empty());
        let no_keymap = matches!(lookup("WASEN_OFFICE_NO_KEYMAP").as_deref(), Some("1"));
        let pdf_probe = lookup("WASEN_OFFICE_PDF_PROBE").is_some_and(|v| !v.is_empty());
        let win_frame = win_frame_lookup(&lookup);
        let web_bridge = matches!(lookup("WASEN_OFFICE_WEB_BRIDGE").as_deref(), Some("1"));

        Self {
            background,
            startup_document,
            devtools,
            boot_mode,
            probe_enabled,
            save_probe_text,
            ux_probe_path,
            nl_probe_marker,
            qv_hittest,
            startpage_probe,
            recents_file,
            saveas_stub,
            untitled_dir,
            compact_chrome,
            tab_probe,
            km_probe,
            no_keymap,
            pdf_probe,
            win_frame,
            web_bridge,
        }
    }

    /// Consume the startup document path if one was supplied (one-shot).
    pub fn take_startup_document(&self) -> Option<String> {
        self.startup_document.lock().ok()?.take()
    }

    /// Calculate the 8-logical-px offscreen sliver position for Rule 6b bg-mode.
    pub fn bg_sliver_position(&self, app: &tauri::AppHandle) -> Option<(f64, f64)> {
        if !self.background {
            return None;
        }
        let m = app.primary_monitor().ok()??;
        let sliver = 8.0;
        Some((
            m.size().width as f64 / m.scale_factor() - sliver,
            m.size().height as f64 / m.scale_factor() - sliver,
        ))
    }

    /// Build probe initialization scripts to inject for the normal boot window.
    pub fn normal_probe_scripts(&self) -> Vec<String> {
        let mut scripts = Vec::new();
        if let Some(path) = &self.ux_probe_path {
            scripts.push(format!(
                "window.__WASEN_OFFICE_UX_PROBE__ = {};",
                serde_json::json!(path)
            ));
        }
        if let Some(text) = &self.save_probe_text {
            scripts.push(format!(
                "window.__WASEN_OFFICE_SAVE_PROBE_TEXT__ = {};",
                serde_json::json!(text)
            ));
        }
        if let Some(probe) = &self.startpage_probe {
            scripts.push(format!(
                "window.__WASEN_OFFICE_STARTPAGE_PROBE__ = {};",
                serde_json::json!(probe)
            ));
        }
        // T-A40 compact chrome + tab spike: plain flags consumed by
        // src/shim/desktopEditor.js (DocsAPI interceptor / probe logging).
        if self.compact_chrome {
            scripts.push("window.__WASEN_OFFICE_COMPACT_CHROME__ = true;".to_string());
        }
        if self.tab_probe {
            scripts.push("window.__WASEN_OFFICE_TAB_PROBE__ = true;".to_string());
        }
        if self.km_probe {
            scripts.push("window.__WASEN_OFFICE_KM_PROBE__ = true;".to_string());
        }
        if self.no_keymap {
            scripts.push("window.__WASEN_OFFICE_NO_KEYMAP__ = true;".to_string());
        }
        if self.pdf_probe {
            scripts.push("window.__WASEN_OFFICE_PDF_PROBE__ = true;".to_string());
        }
        scripts.push(format!(
            "window.__WASEN_OFFICE_PROBE__ = {};",
            self.probe_enabled
        ));
        scripts
    }
}

/// Default open cap in megabytes (T-067, DD open-streaming D4).
const MAX_OPEN_MB_DEFAULT: u64 = 250;
/// One megabyte as the cap unit.
const MB: u64 = 1024 * 1024;

/// Open size cap in bytes — `0` = unlimited
/// (`WASEN_OFFICE_MAX_OPEN_MB`; parse failure falls back to the default,
/// fail-soft — an unwritable env value must never brick opens).
fn max_open_bytes_lookup(lookup: impl Fn(&str) -> Option<String>) -> u64 {
    match lookup("WASEN_OFFICE_MAX_OPEN_MB").and_then(|v| v.trim().parse::<u64>().ok()) {
        Some(mb) => mb.saturating_mul(MB),
        None => MAX_OPEN_MB_DEFAULT * MB,
    }
}

/// Refuse documents above the open cap (T-067 D4/D5) — the one guard text
/// every entry seam reuses. `Ok` = open may proceed.
pub fn ensure_openable_size(path: &std::path::Path) -> Result<(), String> {
    ensure_openable_size_lookup(path, |k| std::env::var(k).ok())
}

/// Pure form of [`ensure_openable_size`] (unit-testable without env races).
fn ensure_openable_size_lookup(
    path: &std::path::Path,
    lookup: impl Fn(&str) -> Option<String>,
) -> Result<(), String> {
    let cap = max_open_bytes_lookup(lookup);
    if cap == 0 {
        return Ok(());
    }
    let size = std::fs::metadata(path)
        .map_err(|e| format!("open guard: metadata: {e}"))?
        .len();
    if size > cap {
        return Err(format!(
            "open guard: file too large to open: {} MB > cap {} MB — opening it \
             would exhaust RAM; raise WASEN_OFFICE_MAX_OPEN_MB (0 = unlimited) \
             if the file fits ({})",
            size / MB,
            cap / MB,
            path.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_default_config_empty_env() {
        let env: HashMap<&str, String> = HashMap::new();
        let cfg = AppConfig::from_lookup(|k| env.get(k).cloned());

        assert!(!cfg.background);
        assert_eq!(cfg.take_startup_document(), None);
        assert!(!cfg.devtools);
        assert_eq!(cfg.boot_mode, BootMode::Normal);
        assert_eq!(cfg.save_probe_text, None);
        assert_eq!(cfg.ux_probe_path, None);
        assert_eq!(cfg.nl_probe_marker, None);
        assert!(!cfg.qv_hittest);
        assert_eq!(cfg.startpage_probe, None);
        assert_eq!(cfg.recents_file, None);
        assert_eq!(cfg.saveas_stub, None);
        assert_eq!(cfg.untitled_dir, None);
        assert!(!cfg.web_bridge);
    }

    /// T-068 web bridge: opt-in by exactly `1`; anything else stays OFF
    /// (release safety — the route must 404 unless a dev asked for it).
    #[test]
    fn test_web_bridge_flag_parsing() {
        for (v, want) in [("1", true), ("0", false), ("", false), ("true", false)] {
            let mut env = HashMap::new();
            env.insert("WASEN_OFFICE_WEB_BRIDGE", v.into());
            let cfg = AppConfig::from_lookup(|k| env.get(k).cloned());
            assert_eq!(cfg.web_bridge, want, "value {v:?}");
        }
    }

    #[test]
    fn test_background_flag_parsing() {
        let mut env = HashMap::new();
        env.insert("WASEN_OFFICE_BACKGROUND", "1".into());
        let cfg = AppConfig::from_lookup(|k| env.get(k).cloned());
        assert!(cfg.background);

        env.insert("WASEN_OFFICE_BACKGROUND", "0".into());
        let cfg = AppConfig::from_lookup(|k| env.get(k).cloned());
        assert!(!cfg.background);

        env.insert("WASEN_OFFICE_BACKGROUND", "".into());
        let cfg = AppConfig::from_lookup(|k| env.get(k).cloned());
        assert!(!cfg.background);
    }

    #[test]
    fn test_boot_mode_precedence() {
        let mut env = HashMap::new();
        env.insert("WASEN_OFFICE_QUICK_VIEW", "/tmp/doc.docx".into());
        let cfg = AppConfig::from_lookup(|k| env.get(k).cloned());
        assert_eq!(cfg.boot_mode, BootMode::QuickView("/tmp/doc.docx".into()));

        // When quick-view is absent, nano-light activates
        env.remove("WASEN_OFFICE_QUICK_VIEW");
        env.insert("WASEN_OFFICE_NANO_LIGHT", "/tmp/nl.docx".into());
        let cfg = AppConfig::from_lookup(|k| env.get(k).cloned());
        assert_eq!(cfg.boot_mode, BootMode::NanoLight("/tmp/nl.docx".into()));
    }

    #[test]
    fn test_probe_script_generation() {
        let mut env = HashMap::new();
        env.insert("WASEN_OFFICE_PROBE", "1".into());
        env.insert("WASEN_OFFICE_SAVE_PROBE", "save-test".into());
        env.insert("WASEN_OFFICE_UX_PROBE", "ux-path.docx".into());
        env.insert("WASEN_OFFICE_STARTPAGE_PROBE", "recent:1".into());
        env.insert("WASEN_OFFICE_RECENTS_FILE", "/tmp/recents-test.json".into());
        let cfg = AppConfig::from_lookup(|k| env.get(k).cloned());

        let scripts = cfg.normal_probe_scripts();
        assert_eq!(scripts.len(), 4);
        assert!(scripts[0].contains("window.__WASEN_OFFICE_UX_PROBE__"));
        assert!(scripts[1].contains("window.__WASEN_OFFICE_SAVE_PROBE_TEXT__"));
        assert!(scripts[2].contains("window.__WASEN_OFFICE_STARTPAGE_PROBE__"));
        assert!(scripts[3].contains("window.__WASEN_OFFICE_PROBE__ = true;"));
        assert_eq!(cfg.recents_file.as_deref(), Some("/tmp/recents-test.json"));
    }

    /// T-A40 knobs (pivoted 18.10 — DD U01 §8): compact chrome DEFAULT OFF
    /// (`=1` opts in); tab probe is presence-triggered like the other probes.
    #[test]
    fn test_compact_chrome_and_tab_probe() {
        // Default (no env): chrome OFF (native engine chrome), probe OFF.
        let cfg = AppConfig::from_lookup(|_| None);
        assert!(!cfg.compact_chrome);
        assert!(!cfg.tab_probe);
        let scripts = cfg.normal_probe_scripts();
        assert_eq!(scripts.len(), 1);
        assert!(scripts[0].contains("window.__WASEN_OFFICE_PROBE__ = true;"));

        // Opt-in + probe presence.
        let mut env = HashMap::new();
        env.insert("WASEN_OFFICE_COMPACT_CHROME", "1".into());
        env.insert("WASEN_OFFICE_TAB_PROBE", "1".into());
        env.insert("WASEN_OFFICE_KM_PROBE", "1".into());
        let cfg = AppConfig::from_lookup(|k| env.get(k).cloned());
        assert!(cfg.compact_chrome);
        assert!(cfg.tab_probe);
        assert!(cfg.km_probe);
        let scripts = cfg.normal_probe_scripts();
        assert_eq!(scripts.len(), 4);
        assert!(scripts[0].contains("window.__WASEN_OFFICE_COMPACT_CHROME__ = true;"));
        assert!(scripts[1].contains("window.__WASEN_OFFICE_TAB_PROBE__ = true;"));
        assert!(scripts[2].contains("window.__WASEN_OFFICE_KM_PROBE__ = true;"));
        assert!(scripts[3].contains("window.__WASEN_OFFICE_PROBE__ = true;"));

        // Empty-string values behave like absence (non-empty discipline).
        let mut env = HashMap::new();
        env.insert("WASEN_OFFICE_COMPACT_CHROME", "".into());
        env.insert("WASEN_OFFICE_TAB_PROBE", "".into());
        let cfg = AppConfig::from_lookup(|k| env.get(k).cloned());
        assert!(!cfg.compact_chrome);
        assert!(!cfg.tab_probe);
    }

    /// Cap knob (T-067 D4): default 250MB; `0` = unlimited; garbage → default.
    #[test]
    fn test_max_open_bytes_parsing() {
        let cl = |env: HashMap<&str, String>| max_open_bytes_lookup(|k| env.get(k).cloned());
        assert_eq!(cl(HashMap::new()), 250 * MB);
        assert_eq!(
            cl([("WASEN_OFFICE_MAX_OPEN_MB", "2".into())]
                .into_iter()
                .collect()),
            2 * MB
        );
        assert_eq!(
            cl([("WASEN_OFFICE_MAX_OPEN_MB", "0".into())]
                .into_iter()
                .collect()),
            0
        );
        assert_eq!(
            cl([("WASEN_OFFICE_MAX_OPEN_MB", "junk".into())]
                .into_iter()
                .collect()),
            250 * MB
        );
        assert_eq!(
            cl([("WASEN_OFFICE_MAX_OPEN_MB", "".into())]
                .into_iter()
                .collect()),
            250 * MB
        );
    }

    /// The guard refuses over-cap files with a self-explanatory message and
    /// passes under-cap files and unlimited mode through.
    #[test]
    fn test_ensure_openable_size() {
        let dir = std::env::temp_dir();
        let f = dir.join(format!("wasen-cap-{}", std::process::id()));
        std::fs::write(&f, [0u8; 10]).unwrap();
        let passthrough: std::collections::HashMap<&str, String> = HashMap::new();
        let no_cap = [("WASEN_OFFICE_MAX_OPEN_MB", "0".into())]
            .into_iter()
            .collect::<std::collections::HashMap<&str, String>>();

        // under-cap file passes (default 250MB cap)
        assert!(ensure_openable_size_lookup(&f, |k| passthrough.get(k).cloned()).is_ok());
        // 1MB cap > 10 bytes — still passes
        let tight = [("WASEN_OFFICE_MAX_OPEN_MB", "1".into())]
            .into_iter()
            .collect::<std::collections::HashMap<&str, String>>();
        assert!(ensure_openable_size_lookup(&f, |k| tight.get(k).cloned()).is_ok());
        // unlimited = always OK
        assert!(ensure_openable_size_lookup(&f, |k| no_cap.get(k).cloned()).is_ok());

        // over-cap → refused, message names the env knob
        let one_byte = [("WASEN_OFFICE_MAX_OPEN_MB", format!("{}", 1))]
            .into_iter()
            .collect::<std::collections::HashMap<&str, String>>();
        std::fs::write(&f, [0u8; 2 * MB as usize]).unwrap();
        let err = ensure_openable_size_lookup(&f, |k| one_byte.get(k).cloned()).unwrap_err();
        assert!(err.contains("file too large to open"), "{err}");
        assert!(err.contains("WASEN_OFFICE_MAX_OPEN_MB"), "{err}");

        // missing file → metadata error, no panic
        let ghost = dir.join("wasen-cap-ghost");
        assert!(ensure_openable_size_lookup(&ghost, |k| passthrough.get(k).cloned()).is_err());

        std::fs::remove_dir(&f).ok();
        std::fs::remove_file(&f).ok();
    }
}
