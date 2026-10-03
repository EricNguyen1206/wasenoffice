//! Viewer command layer (V01, DD `docs/design/viewer-a3/DD.md` §3.1) — the
//! read-only "quick view" plugs in HERE, parallel to delivery: parse-only via
//! the A2 crate, no x2t, no Editor.bin, no FileRegistry. Slice trigger is env
//! `NANO_OFFICE_QUICK_VIEW` (dev-only; UX wiring = V5). The disk path stays
//! Rust-side, like every other document surface.

use serde::Serialize;

use nanooffice_ooxml::Document;

/// Parse-only quick view: read the file, parse it with the A2 crate, hand the
/// model to the viewer window. Anything else (layout, media serving) is a
/// later V-slice — this command must stay parse-only (DD §3.1).
#[tauri::command]
pub fn viewer_open() -> Result<QuickView, String> {
    let path = std::env::var("NANO_OFFICE_QUICK_VIEW")
        .ok()
        .filter(|p| !p.is_empty())
        .ok_or_else(|| "viewer: NANO_OFFICE_QUICK_VIEW is not set".to_string())?;
    let started = std::time::Instant::now();
    let bytes = std::fs::read(&path).map_err(|e| format!("viewer: read {path}: {e}"))?;
    let doc =
        nanooffice_ooxml::parse_docx(&bytes).map_err(|e| format!("viewer: parse {path}: {e}"))?;
    let parse_ms = started.elapsed().as_secs_f64() * 1000.0;
    log::info!(
        "viewer: parsed {path} ({} bytes) in {parse_ms:.1} ms",
        bytes.len()
    );
    Ok(QuickView { parse_ms, doc })
}

/// The quick-view payload: parse time + the raw A2 model (slice V0; V1 swaps
/// in the resolved render model — DD §3.4).
#[derive(Serialize)]
pub struct QuickView {
    pub parse_ms: f64,
    pub doc: Document,
}
