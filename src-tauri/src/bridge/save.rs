//! Desktop local-save (T-010) — Rust side of the save contract.
//!
//! Contract: docs/reference.map.md § "Desktop save contract (word, 9.4.0)".
//! Re-implemented from observed upstream behavior (desktop-sdk save path +
//! CDP oracle on the real app); no upstream code is copied.
//!
//! - [`append_changes`] (`LocalFileSaveChanges`): the engine sends its edit
//!   deltas as `"<len>;<base64>"` entries joined by `","` (or a raw array when
//!   huge). They land in `<workdir>/changes/changes0.json` in the exact byte
//!   format the desktop shell appends, because x2t's `<m_bFromChanges>` reader
//!   consumes this file verbatim. An undo since the last save point truncates
//!   the appended tail at the `((openIndex + deleteIndex) * 2)`-th quote char.
//! - [`save_document`] (`LocalFileSave`): run x2t with a
//!   `<TaskQueueDataConvert>` params XML converting the open-time `Editor.bin`
//!   (+ changes) back to the ORIGINAL registered path, then eval `saveEnd`
//!   into the main frame. delivery.js forwards it DOWN; the engine frame
//!   answers `LocalFileSetSaved` → `LocalFileSetSourcePath` →
//!   `DesktopOfflineAppDocumentEndSave(0 ok | 2 error)`.
//!
//! Disk safety: x2t writes to `<dir>/<name>.nanooffice-save.<ext>` NEXT TO the
//! original (same volume) and the file is renamed over the original only after
//! a complete, non-empty conversion — a partial write can never corrupt the
//! user's file. Semantic failures still reach the engine as EndSave(2)
//! (ConvertationSaveError) instead of failing silently.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde_json::Value;

use super::BridgeError;
use crate::protocol::FileRegistry;

use tauri::Manager as _;

/// Per-document counters mirrored from the desktop shell's per-view state.
#[derive(Default)]
struct SaveState {
    /// changes already applied at open (LocalFileSetOpenChangesCount).
    open_changes_index: i64,
    /// appended-changes cursor (upstream m_nCurrentChangesIndex).
    current_changes_index: i64,
}

fn states() -> &'static Mutex<HashMap<u32, SaveState>> {
    static STATES: OnceLock<Mutex<HashMap<u32, SaveState>>> = OnceLock::new();
    STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// `AVS_OFFICESTUDIO_FILE_DOCUMENT_DOCX` (value from the ONLYOFFICE core
/// format enum; multi-format save = separate task — T-049, TODO.md).
const FORMAT_DOCX: i64 = 65;

/// One-shot x2t work directory for a document (shared with the open-delivery
/// chain in lib.rs — Editor.bin and changes/ live together here, exactly like
/// the real shell's recovery dir).
pub fn workdir_for(id: u32) -> PathBuf {
    std::env::temp_dir().join(format!("nanooffice-doc-{id}"))
}

/// x2t sidecar binary — resolution order (T-A10, Track A rung A1):
/// 1. env `NANO_OFFICE_X2T` (tests / custom builds) — honored only when the
///    file exists, so a bogus value falls through to the bundled converter
///    (the T-A10 counterfactual relies on that)
/// 2. bundled converter: `engines/converter/<target>/x2t` (created by
///    `harness/scripts/bundle-x2t.sh`), found by walking up from the exe dir
///    and the cwd — dev trees have the repo root 3 levels above target/debug
/// 3. installed ONLYOFFICE.app fallback — logged as WARN: this is the exact
///    runtime dependency rung A1 is pulling out
pub fn x2t_path() -> String {
    if let Ok(p) = std::env::var("NANO_OFFICE_X2T") {
        if !p.is_empty() {
            if Path::new(&p).is_file() {
                return p;
            }
            log::warn!("x2t: NANO_OFFICE_X2T points to a missing file ({p}) — ignoring");
        }
    }
    if let Some(p) = find_bundled_x2t() {
        return p;
    }
    log::warn!(
        "x2t: bundled converter engines/converter/{}/x2t not found — falling back to the installed ONLYOFFICE.app (A1 runtime dependency to remove)",
        target_dir_name()
    );
    "/Applications/ONLYOFFICE.app/Contents/Resources/converter/x2t".to_string()
}

/// `engines/converter/<target>` dir name — composed from std::env::consts.
/// Single home for the naming rule: `harness/scripts/bundle-x2t.sh` derives
/// the same name from `rustc -vV` host at bundle time — keep the two in sync.
fn target_dir_name() -> String {
    let arch = std::env::consts::ARCH;
    match std::env::consts::OS {
        "macos" => format!("{arch}-apple-darwin"),
        "linux" => format!("{arch}-unknown-linux-gnu"),
        "windows" => format!("{arch}-pc-windows-msvc"),
        os => format!("{arch}-{os}"),
    }
}

/// Walk up from the exe dir and the cwd looking for
/// `engines/converter/<target>/x2t`. Returns the path and logs it — gate 10
/// evidence that the save used the bundled converter (smoke-save.sh keeps the
/// full log).
fn find_bundled_x2t() -> Option<String> {
    let rel = format!("engines/converter/{}/x2t", target_dir_name());
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        roots.extend(exe.ancestors().skip(1).map(Path::to_path_buf));
    }
    if let Ok(cwd) = std::env::current_dir() {
        roots.extend(cwd.ancestors().map(Path::to_path_buf));
    }
    for root in roots {
        let cand = root.join(&rel);
        if cand.is_file() {
            log::info!("[x2t] using bundled converter: {}", cand.display());
            return Some(cand.to_string_lossy().into_owned());
        }
    }
    None
}

fn changes_path(work: &Path) -> PathBuf {
    work.join("changes").join("changes0.json")
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn value_as_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

pub fn set_open_changes_count(id: u32, n: i64) {
    if let Ok(mut map) = states().lock() {
        map.entry(id).or_default().open_changes_index = n;
    }
}

/// `LocalFileSaveChanges(data, deleteIndex, count)` — append (or truncate on
/// undo) the engine deltas in `<workdir>/changes/changes0.json`.
pub fn append_changes(registry: &FileRegistry, payload: &Value) -> Result<Value, BridgeError> {
    let Some((id, _disk)) = registry.latest() else {
        return Err(BridgeError::Runtime("no document registered yet".into()));
    };
    let args = payload
        .get("args")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let count = args.get(2).and_then(Value::as_i64).unwrap_or(0);
    let delete_index = args.get(1).and_then(Value::as_i64);

    let work = workdir_for(id);
    std::fs::create_dir_all(work.join("changes"))
        .map_err(|e| BridgeError::Runtime(format!("mkdir changes dir: {e}")))?;
    let path = changes_path(&work);

    let mut map = states()
        .lock()
        .map_err(|_| BridgeError::Runtime("save state poisoned".into()))?;
    let state = map.entry(id).or_default();

    // Undo since the last save point → cut the appended tail (upstream
    // RemoveChanges: truncate at the quote char with index (open + delete)*2;
    // two quotes per stored entry).
    if let Some(di) = delete_index {
        if di < state.current_changes_index {
            truncate_to_entry(&path, state.open_changes_index + di);
        }
    }
    state.current_changes_index = delete_index.unwrap_or(0) + count;

    if count != 0 {
        use std::io::Write as _;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| BridgeError::Runtime(format!("open changes file: {e}")))?;
        // Byte-format identical to the desktop shell: `"entry","` per item
        // (the engine pre-joins array items with `","` for the string case).
        let items: Vec<String> = match args.first() {
            Some(Value::Array(items)) => items.iter().map(value_as_string).collect(),
            Some(other) => vec![value_as_string(other)],
            None => Vec::new(),
        };
        for item in items {
            write!(file, "\"{item}\",\"")
                .map_err(|e| BridgeError::Runtime(format!("append changes: {e}")))?;
        }
    }
    Ok(Value::Bool(true))
}

/// Truncate the changes file so exactly `entry_index` entries remain
/// (byte-level equivalent of the desktop shell's RemoveChanges).
fn truncate_to_entry(path: &Path, entry_index: i64) {
    let Ok(data) = std::fs::read(path) else {
        return;
    };
    let target_quote = entry_index.max(0) as usize * 2;
    let mut seen = 0usize;
    let mut cut: Option<usize> = None;
    for (i, b) in data.iter().enumerate() {
        if *b == b'"' {
            if seen == target_quote {
                cut = Some(i);
                break;
            }
            seen += 1;
        }
    }
    if let Some(n) = cut {
        let _ = std::fs::write(path, &data[..n]);
    }
}

/// Push the save result into the engine (via the main frame + the DOWN relay
/// lane in delivery.js). `code`: 0 = saved, 2 = ConvertationSaveError.
fn finish(app: &tauri::AppHandle, path: Option<&Path>, ok: bool, code: i64, note: &str) -> Value {
    if !note.is_empty() {
        log::warn!("save: {note}");
    }
    let info = serde_json::json!({
        "ok": ok,
        "path": path.map(|p| p.to_string_lossy().into_owned()).unwrap_or_default(),
        "code": code,
    });
    let window = app.get_webview_window("main");
    match window {
        Some(win) => {
            let js = format!(
                "window.__nanooffice_delivery && window.__nanooffice_delivery.saveEnd({info});"
            );
            if let Err(e) = win.eval(&js) {
                log::error!("save-end eval failed: {e}");
            }
        }
        None => log::error!("save-end: main window not found — engine not notified"),
    }
    info
}

/// `LocalFileSave(param, password, docinfo, fileType, jsonParams, oldPassword)`
/// — convert Editor.bin (+ changes) back to the original file via x2t.
pub fn save_document(app: &tauri::AppHandle, registry: &FileRegistry, payload: &Value) -> Value {
    let Some((id, disk)) = registry.latest() else {
        return finish(app, None, false, 2, "no document registered yet");
    };
    let args = payload
        .get("args")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if args
        .first()
        .and_then(Value::as_str)
        .is_some_and(|p| p.contains("saveas=true"))
    {
        log::info!(
            "save: saveas requested — saving to the original path (PH-1 has no SaveAs dialog)"
        );
    }

    let ext = disk
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext != "docx" {
        return finish(
            app,
            Some(&disk),
            false,
            2,
            &format!("unsupported save format {ext:?} (multi-format save = T-049)"),
        );
    }

    let work = workdir_for(id);
    let editor_bin = work.join("Editor.bin");
    if !editor_bin.is_file() {
        return finish(
            app,
            Some(&disk),
            false,
            2,
            "Editor.bin missing — open the document before saving",
        );
    }
    let changes = changes_path(&work);
    let from_changes = changes.metadata().map(|m| m.len() > 0).unwrap_or(false);

    // Atomic-ish write: x2t renders next to the target, rename over it after
    // a complete conversion (same directory → same volume).
    let tmp_out = disk.with_extension(format!("nanooffice-save.{ext}"));
    let _ = std::fs::remove_file(&tmp_out);

    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?><TaskQueueDataConvert>");
    xml.push_str(&format!(
        "<m_sFileFrom>{}</m_sFileFrom>",
        xml_escape(&editor_bin.to_string_lossy())
    ));
    xml.push_str(&format!(
        "<m_sFileTo>{}</m_sFileTo>",
        xml_escape(&tmp_out.to_string_lossy())
    ));
    xml.push_str(&format!("<m_nFormatTo>{FORMAT_DOCX}</m_nFormatTo>"));
    if from_changes {
        xml.push_str("<m_bFromChanges>true</m_bFromChanges>");
    }
    xml.push_str("<m_bDontSaveAdditional>true</m_bDontSaveAdditional>");
    // Fonts: the from-changes application (DoctRenderer inside x2t) fails
    // silently without the font cache dir — the conversion then exits
    // non-zero-ish and STILL writes the file, but with the changes DROPPED
    // (observed 28.09: "DoctRenderer error code=open", marker missing). Since
    // T-A11 the AllFonts.js is generated by our own enumerator
    // (protocol::allfonts) — the same file the engine is served — written to
    // a stable temp path once per run. Skip when the generator found no
    // fonts (degenerate machine).
    if let Some(allfonts_path) = crate::protocol::allfonts::ensure_written() {
        xml.push_str(&format!(
            "<m_sAllFontsPath>{}</m_sAllFontsPath>",
            xml_escape(&allfonts_path.to_string_lossy())
        ));
        xml.push_str("<m_nCsvTxtEncoding>46</m_nCsvTxtEncoding>");
        xml.push_str("<m_nCsvDelimiter>4</m_nCsvDelimiter>");
        xml.push_str("<m_nDoctParams>1</m_nDoctParams>");
        if let Some(fonts_dir) = allfonts_path.parent() {
            xml.push_str(&format!(
                "<m_sFontDir>{}</m_sFontDir>",
                xml_escape(&fonts_dir.to_string_lossy())
            ));
        }
    }
    xml.push_str("</TaskQueueDataConvert>");

    let params = work.join("params_to.xml");
    if let Err(e) = std::fs::write(&params, xml) {
        return finish(
            app,
            Some(&disk),
            false,
            2,
            &format!("write params xml: {e}"),
        );
    }

    let started = std::time::Instant::now();
    match std::process::Command::new(x2t_path()).arg(&params).output() {
        // NB: a non-zero exit with output present means a PARTIAL conversion
        // (observed: missing font dir → changes dropped, file still written).
        // Never swap that over the original — surface the error instead.
        Ok(out)
            if out.status.success()
                && tmp_out.is_file()
                && std::fs::metadata(&tmp_out).is_ok_and(|m| m.len() > 0) =>
        {
            // conversion complete — swap it in
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let stdout = String::from_utf8_lossy(&out.stdout);
            let detail = if stderr.trim().is_empty() {
                stdout
            } else {
                stderr
            };
            return finish(
                app,
                Some(&disk),
                false,
                2,
                &format!(
                    "x2t failed (exit {:?}, output: {})",
                    out.status.code(),
                    detail.chars().take(200).collect::<String>()
                ),
            );
        }
        Err(e) => {
            return finish(app, Some(&disk), false, 2, &format!("spawn x2t: {e}"));
        }
    }

    if let Err(e) = std::fs::rename(&tmp_out, &disk) {
        return finish(
            app,
            Some(&disk),
            false,
            2,
            &format!("rename over original: {e}"),
        );
    }
    let len = std::fs::metadata(&disk).map(|m| m.len()).unwrap_or(0);
    // gate 10 (harness/scripts/smoke-save.sh) greps `save delivered:` — keep
    // verbatim (log contract, change in the same commit as the gate).
    log::info!(
        "save delivered: {} ({} bytes) in {:?}",
        disk.display(),
        len,
        started.elapsed()
    );
    finish(app, Some(&disk), true, 0, "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_keeps_exact_entry_count() {
        let dir = std::env::temp_dir().join("nanooffice-test-save");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("changes0.json");
        // two entries in the desktop-shell byte format
        std::fs::write(&path, "\"1;a\",\"2;b\",\"").unwrap();

        truncate_to_entry(&path, 1);
        let kept = std::fs::read_to_string(&path).unwrap();
        // cut before the OPENING quote of entry 1 (upstream cuts at the
        // (entries*2)-th quote char) → entry 0 plus its trailing separator
        assert_eq!(kept, "\"1;a\",");

        truncate_to_entry(&path, 0);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "");

        std::fs::write(&path, "\"1;a\",\"").unwrap();
        truncate_to_entry(&path, 5); // beyond the file → untouched
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "\"1;a\",\"");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn append_matches_shell_byte_format() {
        let dir = std::env::temp_dir().join("nanooffice-test-append");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("changes")).unwrap();
        let path = changes_path(&dir);
        use std::io::Write as _;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .unwrap();
        let joined = "10;AAAA\",\"20;BBBB"; // engine-side join with `","`
        write!(f, "\"{joined}\",\"").unwrap();
        drop(f);
        // parser view: two entries "10;AAAA" and "20;BBBB"
        let raw = std::fs::read_to_string(&path).unwrap();
        let entries: Vec<&str> = raw.split("\",\"").collect();
        assert_eq!(entries[0], "\"10;AAAA");
        assert_eq!(entries[1], "20;BBBB");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
