//! Viewer command layer (V01, DD `docs/design/viewer-a3/DD.md` §3.1) — the
//! read-only "quick view" plugs in HERE, parallel to delivery: parse-only via
//! the A2 crate, no x2t, no Editor.bin, no shim. Two triggers (V5, DD §4):
//! the start page's Quick View action opens a window via `viewer_view`, and
//! the env `NANO_OFFICE_QUICK_VIEW` stays as the dev/gate-12 trigger. The
//! disk path stays Rust-side, like every other document surface.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{State, WebviewUrl, WebviewWindowBuilder};

use nanooffice_ooxml::{Document, Page, RBlock, RenderDoc};

use crate::protocol::FileRegistry;

/// Path handoff from the Quick View action to the window's single
/// `viewer_open` call. `take()` semantics: one stash per open; a second click
/// overwrites before its own window consumes — in practice viewer_open fires
/// on page load, well before the next click (V5, DD §4).
#[derive(Default)]
pub struct QuickViewRequest(Mutex<Option<String>>);

impl QuickViewRequest {
    fn stash(&self, path: String) {
        *self.0.lock().expect("quick-view request mutex poisoned") = Some(path);
    }

    fn take(&self) -> Option<String> {
        self.0
            .lock()
            .expect("quick-view request mutex poisoned")
            .take()
    }
}

/// UI wiring (V5 — DD V01 §4; OQ-V01-3 closed as toggle): validate the path,
/// stash it and open a dedicated viewer window. Refusing here keeps the error
/// on the start page instead of opening a window just to show a failure.
#[tauri::command]
pub fn viewer_view(
    app: tauri::AppHandle,
    quick: State<'_, Arc<QuickViewRequest>>,
    path: String,
) -> Result<(), String> {
    if !std::path::Path::new(&path).is_file() {
        return Err(format!("viewer: no such file: {path}"));
    }
    let label = format!(
        "quickview-{}",
        WINDOW_SEQ.fetch_add(1, Ordering::Relaxed) + 1
    );
    let name = std::path::Path::new(&path)
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.clone());
    quick.stash(path.clone());
    open_surface_window(
        &app,
        &label,
        "viewer.html",
        format!("NanoOffice — Quick View: {name}"),
        "viewer",
    )?;
    log::info!("viewer view: {path} (window {label})");
    Ok(())
}

/// Window-label sequence for UI-opened quick views ("main" = the env boot).
static WINDOW_SEQ: AtomicU32 = AtomicU32::new(0);

/// Shared surface-window body (T-060 C2): the viewer and Nano Light view
/// commands build the same focused(false) window (rule 6b) — label/html/
/// title differ per surface, the error carries the caller's tag. No shim is
/// injected on these windows (no bridge, no engine — DD V01 §3.1, DD L01 §2).
pub(crate) fn open_surface_window(
    app: &tauri::AppHandle,
    label: &str,
    html: &str,
    title: String,
    tag: &str,
) -> Result<(), String> {
    WebviewWindowBuilder::new(app, label, WebviewUrl::App(html.into()))
        .title(title)
        .inner_size(1000.0, 700.0)
        .visible(true)
        .focused(false)
        .build()
        .map_err(|e| format!("{tag}: open window: {e}"))?;
    Ok(())
}

/// The shared open pipeline of the two parse-only surfaces (T-060 C2):
/// read → parse → resolve → OS/2 font metrics → FileRegistry registration
/// (T-059 owner keying — the media route's fileId). `tag` ("viewer" /
/// "nano light") prefixes errors so the gate negative proofs keep biting on
/// the same messages.
pub(crate) struct RenderPayload {
    /// The UNRESOLVED model — the editor session keeps it (write-back-able),
    /// the viewer drops it after resolving.
    pub doc: Document,
    /// The original package bytes — media parts copy byte-exact on save.
    pub bytes: Vec<u8>,
    pub render: RenderDoc,
    pub fonts: BTreeMap<String, [i16; 3]>,
    pub page: Option<Page>,
    pub media_base: Option<String>,
    pub parse_ms: f64,
}

pub(crate) fn render_payload(
    registry: &FileRegistry,
    owner: &str,
    path: &str,
    tag: &str,
) -> Result<RenderPayload, String> {
    let started = std::time::Instant::now();
    let bytes = std::fs::read(path).map_err(|e| format!("{tag}: read {path}: {e}"))?;
    let doc =
        nanooffice_ooxml::parse_docx(&bytes).map_err(|e| format!("{tag}: parse {path}: {e}"))?;
    let render = nanooffice_ooxml::resolve(&doc);
    let fonts: BTreeMap<String, [i16; 3]> = families(&render)
        .into_iter()
        .filter_map(|fam| crate::protocol::allfonts::line_metrics(&fam).map(|m| (fam, m)))
        .collect();
    let media_base = match registry.register(owner, std::path::Path::new(path)) {
        Ok(id) => Some(format!(
            "http://127.0.0.1:{}/media/{id}",
            crate::protocol::filesrv::bound_port()
        )),
        // no registration → no media route; text still renders (the surface
        // must not refuse a document over a serving hiccup)
        Err(e) => {
            log::warn!("{tag}: media disabled — cannot register file: {e}");
            None
        }
    };
    let parse_ms = started.elapsed().as_secs_f64() * 1000.0;
    Ok(RenderPayload {
        page: doc.page.clone(),
        doc,
        bytes,
        render,
        fonts,
        media_base,
        parse_ms,
    })
}

/// Parse-only quick view: the shared open pipeline (T-060 C2 — read → parse
/// → resolve → metrics → registry) plus this surface's log; see
/// `render_payload` for the steps themselves.
#[tauri::command]
pub fn viewer_open(
    registry: State<'_, Arc<FileRegistry>>,
    quick: State<'_, Arc<QuickViewRequest>>,
    window: tauri::WebviewWindow,
) -> Result<QuickView, String> {
    // V5: the UI path stashes the path and the window consumes it; the env
    // stays as the dev/gate-12 trigger (DD §3.1/§4).
    let path = quick
        .take()
        .or_else(|| {
            std::env::var("NANO_OFFICE_QUICK_VIEW")
                .ok()
                .filter(|p| !p.is_empty())
        })
        .ok_or_else(|| {
            "viewer: no quick-view source (UI action or NANO_OFFICE_QUICK_VIEW)".to_string()
        })?;
    let p = render_payload(registry.inner().as_ref(), window.label(), &path, "viewer")?;
    log::info!(
        "viewer: parsed {path} ({} bytes) in {:.1} ms, {} families with OS/2 metrics",
        p.bytes.len(),
        p.parse_ms,
        p.fonts.len()
    );
    Ok(QuickView {
        parse_ms: p.parse_ms,
        doc: p.render,
        fonts: p.fonts,
        page: p.page,
        media_base: p.media_base,
    })
}

/// The quick-view payload: parse time + the RESOLVED render model (V1 — the
/// frontend merges nothing, DD §3.4) + the V2 layout inputs. T-A28 C5: the
/// payload contract is camelCase across surfaces (matches `NanoLightDoc` /
/// `EditOutcome`; the `nano light evidence:` log lines are UNAFFECTED —
/// those keep their frozen gate-12/15 shape).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
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

/// Distinct run font families in document order (deduped) — feeds the
/// shared open pipeline's OS/2 metric lookup.
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
