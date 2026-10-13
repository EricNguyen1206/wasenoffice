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
}

impl AppConfig {
    /// Parse configuration from the live process environment.
    pub fn from_env() -> Self {
        Self::from_lookup(|k| std::env::var(k).ok())
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
        scripts.push(format!(
            "window.__WASEN_OFFICE_PROBE__ = {};",
            self.probe_enabled
        ));
        scripts
    }
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
        let cfg = AppConfig::from_lookup(|k| env.get(k).cloned());

        let scripts = cfg.normal_probe_scripts();
        assert_eq!(scripts.len(), 3);
        assert!(scripts[0].contains("window.__WASEN_OFFICE_UX_PROBE__"));
        assert!(scripts[1].contains("window.__WASEN_OFFICE_SAVE_PROBE_TEXT__"));
        assert!(scripts[2].contains("window.__WASEN_OFFICE_PROBE__ = true;"));
    }
}
