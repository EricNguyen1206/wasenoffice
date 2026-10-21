//! Keymap store (T-A41, DD K02): one JSON store for engine + shell chords.
//! Defaults live here (single home); `keymap.json` holds overrides only —
//! fail-soft: unreadable/garbage file means defaults, never a broken app.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::Manager;

/// One key chord: modifiers + the primary key (letter/digit/`,`)`),
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chord {
    pub cmd: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: String,
}

impl Chord {
    /// `⌘⇧⌥⌃` + key — menu-bar glyph order, macOS native.
    pub fn display(&self) -> String {
        let mut s = String::new();
        if self.cmd {
            s.push('⌘');
        }
        if self.shift {
            s.push('⇧');
        }
        if self.alt {
            s.push('⌥');
        }
        if self.ctrl {
            s.push('⌃');
        }
        s.push_str(&self.key.to_uppercase());
        s
    }

    /// Reveal badge: chord minus the held reveal key (⌘) — `⌘⇧Z` → `⇧Z`.
    pub fn hint(&self) -> String {
        let mut s = String::new();
        if self.ctrl {
            s.push('⌃');
        }
        if self.alt {
            s.push('⌥');
        }
        if self.shift {
            s.push('⇧');
        }
        s.push_str(&self.key.to_uppercase());
        s
    }
}

/// One resolvable keymap row served to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    pub id: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    /// `engine` — applied via sdkjs shortcuts; `shell` — main-frame keydown.
    pub scope: &'static str,
    pub engine_type: Option<u8>,
    pub shell: Option<&'static str>,
    pub chord: Chord,
    pub default_chord: Option<Chord>,
    pub keys: String,
    pub hint: String,
}

/// The 13 v1 actions (DD K02 §5): defaults match MS Office; strikeout empty.
/// (id, label, group, scope, engine_type, shell, default chord)
type DefaultRow = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    Option<u8>,
    Option<&'static str>,
    Option<Chord>,
);

fn defaults() -> Vec<DefaultRow> {
    let c = |key: &str, cmd: bool, shift: bool| Chord {
        cmd,
        ctrl: false,
        shift,
        alt: false,
        key: key.into(),
    };
    vec![
        (
            "bold",
            "In đậm",
            "Định dạng",
            "engine",
            Some(82),
            None,
            Some(c("b", true, false)),
        ),
        (
            "italic",
            "In nghiêng",
            "Định dạng",
            "engine",
            Some(83),
            None,
            Some(c("i", true, false)),
        ),
        (
            "underline",
            "Gạch chân",
            "Định dạng",
            "engine",
            Some(84),
            None,
            Some(c("u", true, false)),
        ),
        (
            "strikeout",
            "Gạch ngang",
            "Định dạng",
            "engine",
            Some(85),
            None,
            None,
        ),
        (
            "save",
            "Lưu",
            "Tài liệu",
            "engine",
            Some(7),
            None,
            Some(c("s", true, false)),
        ),
        (
            "undo",
            "Hoàn tác",
            "Tài liệu",
            "engine",
            Some(53),
            None,
            Some(c("z", true, false)),
        ),
        (
            "redo",
            "Làm lại",
            "Tài liệu",
            "engine",
            Some(54),
            None,
            Some(c("z", true, true)),
        ),
        (
            "selectall",
            "Chọn tất cả",
            "Tài liệu",
            "engine",
            Some(67),
            None,
            Some(c("a", true, false)),
        ),
        (
            "open",
            "Mở file",
            "Ứng dụng",
            "shell",
            None,
            Some("open"),
            Some(c("o", true, false)),
        ),
        (
            "create",
            "Tạo mới",
            "Ứng dụng",
            "shell",
            None,
            Some("create"),
            Some(c("n", true, false)),
        ),
        (
            "settings",
            "Cài đặt",
            "Ứng dụng",
            "shell",
            None,
            Some("settings"),
            Some(c(",", true, false)),
        ),
        (
            "cheatsheet",
            "Bảng phím",
            "Ứng dụng",
            "shell",
            None,
            Some("cheatsheet"),
            Some(c("/", true, false)),
        ),
        (
            "close",
            "Đóng cửa sổ",
            "Ứng dụng",
            "shell",
            None,
            Some("close"),
            Some(c("w", true, false)),
        ),
    ]
}

/// Store path: `<app-data>/keymap.json`; `WASEN_OFFICE_KEYMAP_FILE` overrides
/// (tests/gates — same discipline as `WASEN_OFFICE_RECENTS_FILE`).
fn store_path(app: &tauri::AppHandle) -> PathBuf {
    if let Some(p) = std::env::var("WASEN_OFFICE_KEYMAP_FILE")
        .ok()
        .filter(|v| !v.is_empty())
    {
        return PathBuf::from(p);
    }
    app.path()
        .app_data_dir()
        .map(|dir| dir.join("keymap.json"))
        .unwrap_or_else(|_| std::env::temp_dir().join("wasenoffice-keymap.json"))
}

fn read_overrides(file: &PathBuf) -> HashMap<String, Chord> {
    match std::fs::read_to_string(file) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_else(|e| {
            log::warn!("keymap: garbage store ignored ({e}) — defaults");
            HashMap::new()
        }),
        Err(_) => HashMap::new(),
    }
}

/// Resolved entries: defaults merged with stored overrides.
pub fn load_entries(app: &tauri::AppHandle) -> Vec<Entry> {
    let overrides = read_overrides(&store_path(app));
    defaults()
        .iter()
        .map(
            |(id, label, group, scope, engine_type, shell, default_chord)| {
                let chord = overrides
                    .get(*id)
                    .cloned()
                    .or_else(|| default_chord.clone());
                let (keys, hint) = match &chord {
                    Some(c) => (c.display(), c.hint()),
                    None => ("—".into(), String::new()),
                };
                Entry {
                    id,
                    label,
                    group,
                    scope,
                    engine_type: *engine_type,
                    shell: *shell,
                    chord: chord.unwrap_or(Chord {
                        cmd: false,
                        ctrl: false,
                        shift: false,
                        alt: false,
                        key: String::new(),
                    }),
                    default_chord: default_chord.clone(),
                    keys,
                    hint,
                }
            },
        )
        .collect()
}

/// Persist overrides (called by Settings); returns the resolved entries.
pub fn save_overrides(
    app: &tauri::AppHandle,
    overrides: &HashMap<String, Chord>,
) -> Result<Vec<Entry>, String> {
    let file = store_path(app);
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("keymap: mkdir: {e}"))?;
    }
    let json =
        serde_json::to_string_pretty(overrides).map_err(|e| format!("keymap: encode: {e}"))?;
    // atomic write: tmp + rename, same discipline as the recents store.
    let tmp = file.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| format!("keymap: write: {e}"))?;
    std::fs::rename(&tmp, &file).map_err(|e| format!("keymap: rename: {e}"))?;
    Ok(load_entries(app))
}

/// Frontend: resolved keymap (defaults + overrides).
#[tauri::command]
pub fn keymap_load(app: tauri::AppHandle) -> Vec<Entry> {
    load_entries(&app)
}

/// Frontend: replace overrides wholesale, get back the resolved entries.
#[tauri::command]
pub fn keymap_save(
    app: tauri::AppHandle,
    overrides: HashMap<String, Chord>,
) -> Result<Vec<Entry>, String> {
    save_overrides(&app, &overrides)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_and_hint_glyphs() {
        let c = Chord {
            cmd: true,
            ctrl: false,
            shift: true,
            alt: false,
            key: "z".into(),
        };
        assert_eq!(c.display(), "⌘⇧Z");
        assert_eq!(c.hint(), "⇧Z");
        let plain = Chord {
            cmd: false,
            ctrl: false,
            shift: false,
            alt: false,
            key: "x".into(),
        };
        assert_eq!(plain.display(), "X");
        assert_eq!(plain.hint(), "X");
    }

    #[test]
    fn defaults_have_13_unique_ids() {
        let ids: Vec<_> = defaults().iter().map(|d| d.0).collect();
        assert_eq!(ids.len(), 13);
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), 13);
    }

    /// Fail-soft: garbage store resolves to defaults, never panics (DD §7.5).
    #[test]
    fn garbage_overrides_file_is_ignored() {
        let path =
            std::env::temp_dir().join(format!("wasenoffice-km-test-{}.json", std::process::id()));
        std::fs::write(&path, "{not json").unwrap();
        assert!(read_overrides(&path).is_empty());
        let _ = std::fs::remove_file(&path);
    }
}
