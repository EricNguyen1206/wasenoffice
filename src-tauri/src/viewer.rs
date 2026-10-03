//! Viewer command layer (V01, DD `docs/design/viewer-a3/DD.md` §3.1) — the
//! read-only "quick view" plugs in HERE, parallel to delivery: parse-only via
//! the A2 crate, no x2t, no Editor.bin, no FileRegistry. Slice trigger is env
//! `NANO_OFFICE_QUICK_VIEW` (dev-only; UX wiring = V5). The disk path stays
//! Rust-side, like every other document surface.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use nanooffice_ooxml::{Page, RBlock, RenderDoc};

use crate::protocol::FileRegistry;

/// Parse-only quick view: read the file, parse it with the A2 crate, resolve
/// the style chains (V1 — the crate's `resolve` module, DD V01 §3.2) and hand
/// the RENDER model to the viewer window. V2 added the two layout inputs the
/// frontend cannot get elsewhere: the page geometry (`pgSz`, wrap column) and
/// the OS/2 line metrics per family (C4-a — `allfonts::line_metrics`, bundled
/// set only). V3 registers the file in the FileRegistry so image parts can be
/// served by fileId over the `/media/<id>/<part>` filesrv route — URLs carry
/// no disk paths (DD V01 §3.1; the delivery flow itself is untouched).
#[tauri::command]
pub fn viewer_open(registry: State<'_, Arc<FileRegistry>>) -> Result<QuickView, String> {
    let path = std::env::var("NANO_OFFICE_QUICK_VIEW")
        .ok()
        .filter(|p| !p.is_empty())
        .ok_or_else(|| "viewer: NANO_OFFICE_QUICK_VIEW is not set".to_string())?;
    let started = std::time::Instant::now();
    let bytes = std::fs::read(&path).map_err(|e| format!("viewer: read {path}: {e}"))?;
    let doc =
        nanooffice_ooxml::parse_docx(&bytes).map_err(|e| format!("viewer: parse {path}: {e}"))?;
    let render = nanooffice_ooxml::resolve(&doc);
    let fonts: BTreeMap<String, [i16; 3]> = families(&render)
        .into_iter()
        .filter_map(|fam| crate::protocol::allfonts::line_metrics(&fam).map(|m| (fam, m)))
        .collect();
    let media_base = match registry.register(std::path::Path::new(&path)) {
        Ok(id) => Some(format!(
            "http://127.0.0.1:{}/media/{id}",
            crate::protocol::filesrv::bound_port()
        )),
        // no registration → no media route; text still renders (quick view
        // must not refuse a document over a serving hiccup)
        Err(e) => {
            log::warn!("viewer: media disabled — cannot register file: {e}");
            None
        }
    };
    let parse_ms = started.elapsed().as_secs_f64() * 1000.0;
    log::info!(
        "viewer: parsed {path} ({} bytes) in {parse_ms:.1} ms, {} families with OS/2 metrics",
        bytes.len(),
        fonts.len()
    );
    Ok(QuickView {
        parse_ms,
        doc: render,
        fonts,
        page: doc.page,
        media_base,
    })
}

/// The quick-view payload: parse time + the RESOLVED render model (V1 — the
/// frontend merges nothing, DD §3.4) + the V2 layout inputs.
#[derive(Serialize)]
pub struct QuickView {
    pub parse_ms: f64,
    pub doc: RenderDoc,
    /// OS/2 line metrics (C4-a) per family found in the document:
    /// `[ascent, descent, line_gap]` scaled 1000/em. Families without a
    /// bundled match are absent → the renderer applies its fallback spacing.
    pub fonts: BTreeMap<String, [i16; 3]>,
    /// `w:pgSz` (§17.6.11) of the final section — the wrap column's source.
    pub page: Option<Page>,
    /// V3: origin + fileId prefix for image parts — `<media_base>/<OPC part>`
    /// fetches those bytes from the loopback filesrv. `None` when the file
    /// could not be registered.
    pub media_base: Option<String>,
}

/// Distinct run font families in document order (deduped).
fn families(doc: &RenderDoc) -> Vec<String> {
    fn walk_blocks(blocks: &[RBlock], out: &mut Vec<String>) {
        for b in blocks {
            match b {
                RBlock::Paragraph(p) => {
                    for r in &p.runs {
                        if let Some(f) = &r.font {
                            if !out.iter().any(|s| s == f) {
                                out.push(f.clone());
                            }
                        }
                    }
                }
                RBlock::Table(t) => {
                    for row in &t.rows {
                        for cell in row {
                            walk_blocks(&cell.blocks, out);
                        }
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    walk_blocks(&doc.body, &mut out);
    out
}
