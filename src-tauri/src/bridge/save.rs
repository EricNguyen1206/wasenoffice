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
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use serde_json::Value;

use super::BridgeError;
use crate::formats::{DocWorkdir, CSV_DELIMITER, CSV_TXT_ENCODING, DOCT_PARAMS};
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

// Format facts (x2t codes, save eligibility) + the workdir layout live in the
// registry (`crate::formats` — T-052); converter spawn lives in the sidecar
// seam (`crate::sidecar` — T-A14). This module keeps the changes journal and
// the save orchestration.

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// The save `<TaskQueueDataConvert>` params XML — pure so gate 16 exercises the app's exact bytes; `format_to=None` = extension fallback (reference.map § multi-format).
fn save_params_xml(
    editor_bin: &Path,
    out: &Path,
    format_to: Option<i64>,
    from_changes: bool,
    allfonts_path: Option<&Path>,
) -> String {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?><TaskQueueDataConvert>");
    xml.push_str(&format!(
        "<m_sFileFrom>{}</m_sFileFrom>",
        xml_escape(&editor_bin.to_string_lossy())
    ));
    xml.push_str(&format!(
        "<m_sFileTo>{}</m_sFileTo>",
        xml_escape(&out.to_string_lossy())
    ));
    if let Some(code) = format_to {
        xml.push_str(&format!("<m_nFormatTo>{code}</m_nFormatTo>"));
    }
    if from_changes {
        xml.push_str("<m_bFromChanges>true</m_bFromChanges>");
    }
    xml.push_str("<m_bDontSaveAdditional>true</m_bDontSaveAdditional>");
    // no font params = DoctRenderer silently drops the changes (obs 28.09) — omit only on a degenerate machine
    if let Some(allfonts) = allfonts_path {
        xml.push_str(&format!(
            "<m_sAllFontsPath>{}</m_sAllFontsPath>",
            xml_escape(&allfonts.to_string_lossy())
        ));
        xml.push_str(&format!(
            "<m_nCsvTxtEncoding>{CSV_TXT_ENCODING}</m_nCsvTxtEncoding>"
        ));
        xml.push_str(&format!(
            "<m_nCsvDelimiter>{CSV_DELIMITER}</m_nCsvDelimiter>"
        ));
        xml.push_str(&format!("<m_nDoctParams>{DOCT_PARAMS}</m_nDoctParams>"));
        if let Some(fonts_dir) = allfonts.parent() {
            xml.push_str(&format!(
                "<m_sFontDir>{}</m_sFontDir>",
                xml_escape(&fonts_dir.to_string_lossy())
            ));
        }
    }
    xml.push_str("</TaskQueueDataConvert>");
    xml
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

/// T-059: the engine call's OWN document — the shim stamps every relayed call
/// with the `fileId` its window received from `open_document`; an unknown or
/// missing id is a loud error instead of a silent "whatever registered last".
fn flow_file_id(
    registry: &FileRegistry,
    payload: &Value,
) -> Result<(u32, std::path::PathBuf), String> {
    let id = payload
        .get("fileId")
        .and_then(Value::as_u64)
        .map(|v| v as u32)
        .ok_or_else(|| "engine call without fileId (shim must stamp the open id)".to_string())?;
    let disk = registry
        .get(id)
        .ok_or_else(|| format!("no document registered for fileId {id}"))?;
    Ok((id, disk))
}

/// `LocalFileSaveChanges(data, deleteIndex, count)` — append (or truncate on
/// undo) the engine deltas in `<workdir>/changes/changes0.json`.
pub fn append_changes(registry: &FileRegistry, payload: &Value) -> Result<Value, BridgeError> {
    let (id, _disk) = flow_file_id(registry, payload).map_err(BridgeError::Runtime)?;
    let args = payload
        .get("args")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let count = args.get(2).and_then(Value::as_i64).unwrap_or(0);
    let delete_index = args.get(1).and_then(Value::as_i64);

    let work = DocWorkdir::new(id);
    std::fs::create_dir_all(work.changes_dir())
        .map_err(|e| BridgeError::Runtime(format!("mkdir changes dir: {e}")))?;
    let path = work.changes_file();

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
    let (id, disk) = match flow_file_id(registry, payload) {
        Ok(id_disk) => id_disk,
        Err(note) => return finish(app, None, false, 2, &note),
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
    let format_to = match crate::formats::by_ext(&ext) {
        Some(f) if f.save.is_wired() => f.wired_save_code(),
        _ => {
            return finish(
                app,
                Some(&disk),
                false,
                2,
                &format!("unsupported save format {ext:?}"),
            )
        }
    };

    let work = DocWorkdir::new(id);
    let editor_bin = work.editor_bin();
    if !editor_bin.is_file() {
        return finish(
            app,
            Some(&disk),
            false,
            2,
            "Editor.bin missing — open the document before saving",
        );
    }
    let changes = work.changes_file();
    let from_changes = changes.metadata().map(|m| m.len() > 0).unwrap_or(false);

    // Atomic write (T-062): x2t renders to temporary sibling, then atomic swap.
    let tmp_out = crate::storage::sibling_tmp_path(&disk, "nanooffice-save");
    let _ = std::fs::remove_file(&tmp_out);

    let allfonts = crate::protocol::allfonts::ensure_written();
    let xml = save_params_xml(
        &editor_bin,
        &tmp_out,
        format_to,
        from_changes,
        allfonts.as_deref(),
    );

    let params = work.params_xml();
    if let Err(e) = std::fs::write(&params, xml) {
        return finish(
            app,
            Some(&disk),
            false,
            2,
            &format!("write params xml: {e}"),
        );
    }

    // Success = exit 0 + non-empty output (a non-zero exit with output
    // present means a PARTIAL conversion — never swap that over the original;
    // observed: missing font dir → changes dropped, file still written).
    // The criterion + spawn + output capture live in the sidecar seam.
    let conv = match crate::sidecar::convert(&crate::sidecar::ConvertRequest::SaveFromChanges {
        params_xml: &params,
        output: &tmp_out,
    }) {
        Ok(conv) => conv, // conversion complete — swap it in
        Err(e) => return finish(app, Some(&disk), false, 2, &e.note),
    };

    let len = match crate::storage::atomic_swap(&tmp_out, &disk) {
        Ok(len) => len,
        Err(e) => return finish(app, Some(&disk), false, 2, &e),
    };
    // gate 10 (harness/scripts/smoke-save.sh) greps `save delivered:` — keep
    // verbatim (log contract, change in the same commit as the gate).
    log::info!(
        "save delivered: {} ({} bytes) in {:?}",
        disk.display(),
        len,
        conv.elapsed
    );
    finish(app, Some(&disk), true, 0, "")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T-059 race: open A ("main") then register B ("quickview-1") — the
    /// old `latest()` keying would hand A's save flow B's workdir. Every
    /// engine call must land in the workdir of the id IT carries.
    #[test]
    fn save_flows_key_by_the_file_id_they_carry() {
        use crate::formats::DocWorkdir;
        let dir = std::env::temp_dir().join("nanooffice-test-save-keyed");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.docx");
        let b = dir.join("b.docx");
        std::fs::write(&a, b"document A").unwrap();
        std::fs::write(&b, b"document B").unwrap();

        let registry = FileRegistry::default();
        let id_a = registry.register("main", &a).unwrap();
        let id_b = registry.register("quickview-1", &b).unwrap();
        assert!(id_a < id_b);

        // deltas for A land in A's workdir, even though B registered later
        let payload = serde_json::json!({ "fileId": id_a, "args": ["10;AAAA", 0, 1] });
        append_changes(&registry, &payload).expect("append keyed by fileId");
        assert!(
            DocWorkdir::new(id_a).changes_file().is_file(),
            "changes must land in the carried id's workdir"
        );
        assert!(
            !DocWorkdir::new(id_b).changes_dir().exists(),
            "the later-registered file must not be touched"
        );

        // counterfactual: missing / unknown fileId fail loudly
        assert!(append_changes(&registry, &serde_json::json!({ "args": ["x", 0, 1] })).is_err());
        assert!(append_changes(
            &registry,
            &serde_json::json!({ "fileId": id_b + 7, "args": ["x", 0, 1] })
        )
        .is_err());

        // LocalFileSetOpenChangesCount keying shares the same contract
        let (id, disk) = flow_file_id(&registry, &serde_json::json!({ "fileId": id_a })).unwrap();
        assert_eq!(id, id_a);
        assert!(disk.ends_with("a.docx"));
        let _ = std::fs::remove_dir_all(DocWorkdir::new(id_a).root());
        let _ = std::fs::remove_dir_all(DocWorkdir::new(id_b).root());
        let _ = std::fs::remove_dir_all(&dir);
    }

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

    /// Params shape per save kind: xlsx carries 257, pptx carries NO code (fallback), fonts conditional.
    #[test]
    fn save_multiformat_params_xml_shape() {
        let fonts = Path::new("/tmp/nf/AllFonts.js");
        let xlsx = save_params_xml(
            Path::new("/w/Editor.bin"),
            Path::new("/o/f.xlsx"),
            crate::formats::by_ext("xlsx").unwrap().wired_save_code(),
            true,
            Some(fonts),
        );
        assert!(xlsx.contains("<m_nFormatTo>257</m_nFormatTo>"));
        assert!(xlsx.contains("<m_bFromChanges>true</m_bFromChanges>"));
        assert!(xlsx.contains("<m_sFontDir>/tmp/nf</m_sFontDir>"));
        assert!(xlsx.contains(&format!("<m_nCsvTxtEncoding>{CSV_TXT_ENCODING}")));

        let pptx = save_params_xml(
            Path::new("/w/Editor.bin"),
            Path::new("/o/f.pptx"),
            crate::formats::by_ext("pptx").unwrap().wired_save_code(),
            false,
            Some(fonts),
        );
        assert!(!pptx.contains("m_nFormatTo"), "fallback = no format code");
        assert!(!pptx.contains("m_bFromChanges"));

        // degenerate machine (no fonts) → the whole font block stays out
        let bare = save_params_xml(
            Path::new("/w/Editor.bin"),
            Path::new("/o/f.docx"),
            Some(65),
            false,
            None,
        );
        assert!(!bare.contains("m_sAllFontsPath"));
        assert!(bare.contains("<m_nFormatTo>65</m_nFormatTo>"));
    }

    /// Fixture bytes from the repo generators (the same ones the behavioral gates use).
    fn generate_fixture(ext: &str, dir: &Path) -> std::path::PathBuf {
        let gen = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../harness/fixtures/make_{ext}.py"));
        let out = dir.join(format!("fixture.{ext}"));
        let status = std::process::Command::new("python3")
            .arg(&gen)
            .arg(&out)
            .status()
            .expect("spawn python3 for fixture generation");
        assert!(status.success(), "make_{ext}.py failed");
        out
    }

    fn multiformat_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("nanooffice-test-multiformat-{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Gate 16 positive: fixture → Editor.bin → save params (the app's exact builder) → real OOXML zip, per format.
    #[test]
    fn save_multiformat_roundtrip_xlsx_pptx() {
        let x2t = crate::sidecar::x2t_path();
        assert!(
            Path::new(&x2t).is_file(),
            "x2t not resolvable ({x2t:?}) — bundle it or set NANO_OFFICE_X2T"
        );
        let dir = multiformat_dir("rt");
        // production save params carry the generated font cache
        let allfonts = crate::protocol::allfonts::ensure_written();
        for (i, (ext, root_part)) in [
            ("xlsx", "xl/workbook.xml"),
            ("pptx", "ppt/presentation.xml"),
        ]
        .iter()
        .enumerate()
        {
            let src = generate_fixture(ext, &dir);
            let work = DocWorkdir::new(987_650 + i as u32);
            std::fs::create_dir_all(work.root()).unwrap();
            let editor_bin = work.editor_bin();
            crate::sidecar::convert(&crate::sidecar::ConvertRequest::OpenBinary {
                src: &src,
                dst: &editor_bin,
            })
            .unwrap_or_else(|e| panic!("open {ext} → Editor.bin: {e}"));
            let out = dir.join(format!("saved.{ext}"));
            let params = dir.join(format!("params-{ext}.xml"));
            std::fs::write(
                &params,
                save_params_xml(
                    &editor_bin,
                    &out,
                    crate::formats::by_ext(ext).unwrap().wired_save_code(),
                    false,
                    allfonts.as_deref(),
                ),
            )
            .unwrap();
            crate::sidecar::convert(&crate::sidecar::ConvertRequest::SaveFromChanges {
                params_xml: &params,
                output: &out,
            })
            .unwrap_or_else(|e| panic!("save {ext}: {e}"));
            let file = std::fs::File::open(&out).unwrap();
            let mut zip = zip::ZipArchive::new(file).expect("saved file is a zip");
            assert!(
                zip.by_name(root_part).is_ok(),
                "saved {ext} must contain {root_part}"
            );
            let _ = std::fs::remove_dir_all(work.root());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// T-049 gate 16 (negative): garbage input must fail the save loudly, and
    /// pdf stays refused before any converter spawn.
    #[test]
    fn save_multiformat_negative_garbage_fails() {
        let x2t = crate::sidecar::x2t_path();
        assert!(
            Path::new(&x2t).is_file(),
            "x2t not resolvable ({x2t:?}) — bundle it or set NANO_OFFICE_X2T"
        );
        let dir = multiformat_dir("neg");
        let bin = dir.join("Editor.bin");
        std::fs::write(&bin, b"definitely not an editor binary").unwrap();
        let out = dir.join("out.xlsx");
        let params = dir.join("params.xml");
        std::fs::write(&params, save_params_xml(&bin, &out, Some(257), false, None)).unwrap();
        assert!(
            crate::sidecar::convert(&crate::sidecar::ConvertRequest::SaveFromChanges {
                params_xml: &params,
                output: &out,
            })
            .is_err(),
            "garbage Editor.bin must not yield a successful save"
        );
        assert!(!crate::formats::by_ext("pdf").unwrap().save.is_wired());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn append_matches_shell_byte_format() {
        let dir = std::env::temp_dir().join("nanooffice-test-append");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("changes")).unwrap();
        // layout pinned by formats::DocWorkdir tests
        let path = dir.join("changes").join("changes0.json");
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
