//! Untitled documents — Create-new + Save-As (A01 T-A39, DD §5, Q7/Q8).
//!
//! Create-new copies a bundled Template blank into `<app-data>/untitled/` and
//! opens it through the normal pipeline: a REAL file, so every existing gate
//! (registry canonicalize, x2t open) works unchanged. The UNTITLED marker
//! lives on the registry entry; the FIRST save intercepts in
//! `bridge::save::save_document` → Save-As panel (Q8) → writes to the chosen
//! path, re-keys the registry entry, removes the untitled copy, refreshes the
//! window title and records the recent. `WASEN_OFFICE_SAVEAS_STUB` replaces
//! the panel for tests/gates (NSSavePanel is not scriptable). Nano Light never
//! holds untitled docs (create-new only lands in the engine surface), so its
//! save path is untouched.
//!
//! Template provenance (clean-room record): hand-built docx per ECMA-376 /
//! openpyxl (MIT) xlsx / python-pptx (MIT) pptx — generator
//! `harness/fixtures/make_blank_templates.py`, artifacts committed under
//! `src-tauri/resources/templates/`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

use crate::config::AppConfig;
use crate::protocol::FileRegistry;

/// Template blank bytes embedded in the binary (~33 KB total) — works the
/// same in dev and in the bundled app, no resource-path resolution.
pub fn template_bytes(ext: &str) -> Option<&'static [u8]> {
    match ext {
        "docx" => Some(include_bytes!("../resources/templates/blank.docx").as_slice()),
        "xlsx" => Some(include_bytes!("../resources/templates/blank.xlsx").as_slice()),
        "pptx" => Some(include_bytes!("../resources/templates/blank.pptx").as_slice()),
        _ => None,
    }
}

/// Copy the template into `<dir>/Untitled <n>.<ext>` — first free `n`.
pub fn create_document_file(dir: &Path, kind: &str) -> Result<PathBuf, String> {
    let ext = kind.trim().trim_start_matches('.').to_ascii_lowercase();
    let bytes = template_bytes(&ext)
        .ok_or_else(|| format!("create: unsupported kind {kind:?} (docx/xlsx/pptx)"))?;
    std::fs::create_dir_all(dir).map_err(|e| format!("mkdir {}: {e}", dir.display()))?;
    let mut n = 1;
    loop {
        let path = dir.join(format!("Untitled {n}.{ext}"));
        if !path.exists() {
            std::fs::write(&path, bytes).map_err(|e| format!("write {}: {e}", path.display()))?;
            return Ok(path);
        }
        n += 1;
    }
}

fn untitled_dir(app: &tauri::AppHandle) -> PathBuf {
    if let Some(config) = app.try_state::<Arc<AppConfig>>() {
        if let Some(dir) = config.untitled_dir.as_deref() {
            return PathBuf::from(dir);
        }
    }
    app.path()
        .app_data_dir()
        .map(|d| d.join("untitled"))
        .unwrap_or_else(|e| {
            log::warn!("untitled: app data dir: {e} — falling back to temp");
            std::env::temp_dir().join("wasenoffice-untitled")
        })
}

/// UI wiring: copy a Template blank and register it as an UNTITLED document
/// of the calling window. Deliberately does NOT record a recent — the untitled
/// copy is app-managed and disappears at first save (DD §6).
#[tauri::command]
pub fn create_document(
    window: tauri::WebviewWindow,
    kind: String,
    registry: tauri::State<'_, Arc<FileRegistry>>,
) -> Result<crate::OpenedDocument, String> {
    let dir = untitled_dir(window.app_handle());
    let path = create_document_file(&dir, &kind)?;
    log::info!("create_document: {kind} → {}", path.display());
    let doc = crate::register_and_url(&window, path.to_string_lossy().as_ref(), &registry)?;
    registry.mark_untitled(doc.file_id);
    Ok(doc)
}

/// Resolve the save target for an UNTITLED document: the stub wins when set
/// (tests/gates), otherwise the platform Save panel prefilled with the
/// untitled name, starting in the user's Documents dir. `Err` = canceled or
/// dialog failure — the caller aborts the save (DD §6: nothing is lost).
pub fn save_as_target(app: &tauri::AppHandle, current: &Path) -> Result<PathBuf, String> {
    if let Some(config) = app.try_state::<Arc<AppConfig>>() {
        if let Some(stub) = config.saveas_stub.as_deref() {
            return Ok(PathBuf::from(stub));
        }
    }
    let name = current
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Untitled.docx".into());
    let mut dialog = app.dialog().file().set_file_name(&name);
    if let Ok(dir) = app.path().document_dir() {
        dialog = dialog.set_directory(dir);
    }
    match dialog
        .add_filter("Documents", &["docx", "xlsx", "pptx", "pdf"])
        .blocking_save_file()
    {
        Some(picked) => picked.into_path().map_err(|e| format!("save-as path: {e}")),
        None => Err("save canceled — untitled document kept".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "wasenoffice-untitled-test-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn untitled_create_names_seq_and_copies_bytes() {
        let dir = temp_dir("seq");
        let a = create_document_file(&dir, "docx").unwrap();
        let b = create_document_file(&dir, "docx").unwrap();
        assert_eq!(a.file_name().unwrap(), "Untitled 1.docx");
        assert_eq!(b.file_name().unwrap(), "Untitled 2.docx");
        assert_eq!(
            std::fs::read(&a).unwrap(),
            template_bytes("docx").unwrap(),
            "untitled copy is byte-exact with the bundled template"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn untitled_create_rejects_unknown_kind() {
        let dir = temp_dir("reject");
        assert!(create_document_file(&dir, "exe").is_err());
        assert!(create_document_file(&dir, "").is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn untitled_templates_are_valid_ooxml_packages() {
        // zip is a dev-dependency (T-049) — assert the root part each format
        // is required to carry (formats::FORMATS inner-part contract).
        for (ext, root_part) in [
            ("docx", "word/document.xml"),
            ("xlsx", "xl/workbook.xml"),
            ("pptx", "ppt/presentation.xml"),
        ] {
            let bytes = template_bytes(ext).unwrap();
            assert!(bytes.starts_with(b"PK\x03\x04"), "{ext}: not a zip package");
            let cursor = std::io::Cursor::new(bytes);
            let mut archive = zip::ZipArchive::new(cursor).unwrap();
            assert!(
                archive.by_name(root_part).is_ok(),
                "{ext}: missing root part {root_part}"
            );
        }
    }

    /// Gate oracle (pattern gate 16 — real x2t, no app boot): the ENGINE must
    /// accept every bundled blank through the exact open shape the shell uses
    /// (template → Editor.bin). A template x2t rejects would brick Create-new.
    #[test]
    fn test_untitled_templates_open_via_x2t() {
        let x2t = crate::sidecar::x2t_path();
        assert!(
            Path::new(&x2t).is_file(),
            "x2t not resolvable ({x2t:?}) — bundle it or set WASEN_OFFICE_X2T"
        );
        let dir = temp_dir("x2t");
        for ext in ["docx", "xlsx", "pptx"] {
            let src = create_document_file(&dir, ext).unwrap();
            let dst = dir.join(format!("{ext}-editor.bin"));
            let conv = crate::sidecar::convert(&crate::sidecar::ConvertRequest::OpenBinary {
                src: &src,
                dst: &dst,
            })
            .unwrap_or_else(|e| panic!("{ext}: x2t rejected the blank template: {}", e.note));
            let len = std::fs::metadata(&dst).map(|m| m.len()).unwrap_or(0);
            assert!(len > 0, "{ext}: empty Editor.bin");
            log::info!(
                "untitled: x2t accepted blank.{ext} → {len} B Editor.bin in {:?}",
                conv.elapsed
            );
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}
