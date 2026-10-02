//! Custom protocols serving the engine and user documents to the WebView.
//!
//! - `nanooffice-engine://` — engine assets (`editors/…`), generated `AllFonts.js`
//!   and font files. The editor `index.html` gets a fonts `<script>` injected at
//!   serve time (the desktop app does the equivalent natively via CEF).
//! - `nanooffice-file://` — user documents: `file/<id>/<name>` URLs resolved
//!   through [`FileRegistry`]; the JS side never sees disk paths.
//!
//! Both handlers support HTTP `Range` requests (embedded media in large files)
//! and answer with `Access-Control-Allow-Origin: *` (the editor iframe origin
//! differs from the start page / document scheme).
//! See harness/skills/open-docx/SKILL.md for the open/save flow.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use tauri::http::{header, Request, Response, StatusCode};
use tauri::Manager;

pub mod filesrv;

pub(crate) mod allfonts;

pub const ENGINE_SCHEME: &str = "nanooffice-engine";
pub const FILE_SCHEME: &str = "nanooffice-file";

// T-054: the font-root table (`FONT_URL_ROOTS`/`font_root_disk`), engines
// discovery (`engines_root`), URI-component encoding and BOTH AllFonts.js
// flavors (disk `js_raw` / loopback-URL `engine_flavor_js`) live in
// `protocol::allfonts` — the producer. This module and filesrv are consumers.

/// fileId → absolute path registry. Kept in Rust state; JS only ever sees
/// `…/file/<id>/<name>` URLs (custom scheme + loopback HTTP mirror).
#[derive(Default)]
pub struct FileRegistry {
    next_id: AtomicU32,
    entries: Mutex<HashMap<u32, PathBuf>>,
}

impl FileRegistry {
    pub fn register(&self, path: &Path) -> Result<u32, String> {
        let canonical = path
            .canonicalize()
            .map_err(|e| format!("cannot resolve {}: {e}", path.display()))?;
        if !canonical.is_file() {
            return Err(format!("not a regular file: {}", canonical.display()));
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst) + 1;
        self.entries
            .lock()
            .map_err(|_| "registry poisoned".to_string())?
            .insert(id, canonical);
        Ok(id)
    }

    pub fn get(&self, id: u32) -> Option<PathBuf> {
        self.entries.lock().ok()?.get(&id).cloned()
    }

    /// (id, path) of the most recently registered file (PH-0 single-doc flow).
    pub fn latest(&self) -> Option<(u32, PathBuf)> {
        let entries = self.entries.lock().ok()?;
        let id = entries.keys().copied().max()?;
        entries.get(&id).cloned().map(|p| (id, p))
    }
}

// ---------------------------------------------------------------------------
// request handling helpers
// ---------------------------------------------------------------------------

/// Resolved byte range of a response body (inclusive end).
pub(crate) struct RangeSpec {
    pub(crate) start: u64,
    pub(crate) end: u64, // inclusive
    pub(crate) partial: bool,
}

/// Parse a `Range` header VALUE (`bytes=…` or None) against a body length.
/// Shared by the custom-scheme handler and the loopback filesrv (T-054:
/// filesrv used to carry an untested re-implementation — both now go through
/// this one, which has the tests below).
pub(crate) fn parse_range_header(spec: Option<&str>, len: u64) -> RangeSpec {
    let full = RangeSpec {
        start: 0,
        end: len.saturating_sub(1),
        partial: false,
    };
    if len == 0 {
        return full;
    }
    let Some(spec) = spec.map(str::trim) else {
        return full;
    };
    let Some(spec) = spec.strip_prefix("bytes=") else {
        return full;
    };
    let Some((start_s, end_s)) = spec.split_once('-') else {
        return full;
    };
    let (start, end) = if start_s.is_empty() {
        // suffix range: last N bytes
        let Ok(n) = end_s.parse::<u64>() else {
            return full;
        };
        if n == 0 || n > len {
            return full;
        }
        (len - n, len - 1)
    } else {
        let Ok(start) = start_s.parse::<u64>() else {
            return full;
        };
        let end = if end_s.is_empty() {
            len - 1
        } else {
            end_s.parse::<u64>().unwrap_or(len - 1).min(len - 1)
        };
        if start > end {
            return full;
        }
        (start, end)
    };
    RangeSpec {
        start,
        end,
        partial: true,
    }
}

fn parse_range(headers: &tauri::http::HeaderMap, len: u64) -> RangeSpec {
    parse_range_header(
        headers.get(header::RANGE).and_then(|v| v.to_str().ok()),
        len,
    )
}

fn mime_for(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "application/javascript",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "wasm" => "application/wasm",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "ttc" => "font/collection",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "pdf" => "application/pdf",
        "txt" | "log" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn empty(code: StatusCode) -> Response<Vec<u8>> {
    Response::builder()
        .status(code)
        .body(Vec::new())
        .expect("static response")
}

fn bytes_response(
    code: StatusCode,
    range: &RangeSpec,
    total_len: u64,
    mime: &str,
    body: Vec<u8>,
    content_disposition: Option<String>,
) -> Response<Vec<u8>> {
    let mut builder = Response::builder()
        .status(code)
        .header(header::CONTENT_TYPE, mime)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::CONTENT_LENGTH, body.len().to_string());
    if range.partial {
        builder = builder.header(
            header::CONTENT_RANGE,
            format!("bytes {}-{}/{total_len}", range.start, range.end),
        );
    }
    if let Some(cd) = content_disposition {
        builder = builder.header(header::CONTENT_DISPOSITION, cd);
    }
    builder.body(body).expect("response with body")
}

/// Serve a (possibly ranged) file from disk.
fn serve_file(
    path: &Path,
    request: &Request<Vec<u8>>,
    content_disposition: Option<String>,
) -> tauri::http::Response<Vec<u8>> {
    let Ok(mut file) = File::open(path) else {
        log::warn!("protocol 404: {}", path.display());
        return empty(StatusCode::NOT_FOUND);
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let range = parse_range(request.headers(), len);
    let mime = mime_for(&path.to_string_lossy());

    let buf = if range.partial {
        let mut buf = vec![0u8; (range.end - range.start + 1) as usize];
        if file.seek(SeekFrom::Start(range.start)).is_ok() {
            // Tolerate short reads (file changed mid-request) — engine retries.
            let _ = file.read_exact(&mut buf);
        }
        buf
    } else {
        let mut buf = Vec::with_capacity(len as usize);
        let _ = file.read_to_end(&mut buf);
        buf
    };

    let code = if range.partial {
        StatusCode::PARTIAL_CONTENT
    } else {
        StatusCode::OK
    };
    bytes_response(code, &range, len, mime, buf, content_disposition)
}

/// Reject anything escaping its root (no `..`, no absolute components).
fn safe_join(root: &Path, rel: &str) -> Option<PathBuf> {
    let rel_path = Path::new(rel);
    if rel.is_empty()
        || rel_path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return None;
    }
    Some(root.join(rel_path))
}

// ---------------------------------------------------------------------------
// percent-encoding — request path DECODING only; the ENCODER
// (`encode_uri_component`) lives in protocol::allfonts since T-054, next to
// the URL producer that needs it (one home, no drift between the two).
// ---------------------------------------------------------------------------

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 3 <= bytes.len() {
            if let Ok(v) = u8::from_str_radix(&String::from_utf8_lossy(&bytes[i + 1..i + 3]), 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ---------------------------------------------------------------------------
// protocol handlers (tauri entry points)
// ---------------------------------------------------------------------------

pub fn handle_engine<R: tauri::Runtime>(
    _ctx: tauri::UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    let Some(engines) = allfonts::engines_root() else {
        log::error!("engines/ root not found — set NANO_OFFICE_ENGINES or run from the repo");
        return empty(StatusCode::NOT_FOUND);
    };
    // WKWebView normalizes custom scheme URLs to <scheme>://localhost/<path>;
    // match on the path only (host-agnostic).
    let path = request.uri().path().trim_start_matches('/');
    let path = percent_decode(path);
    log::debug!("engine GET {path}");

    // generated font index — the engine fetches it via requirejs from the
    // sdkjs tree (the desktop app serves this path from its native side; we
    // have no on-disk file, only the generator).
    if path == "editors/sdkjs/common/AllFonts.js" {
        return match allfonts::engine_flavor_js(filesrv::bound_port()) {
            Ok(bytes) => {
                let len = bytes.len() as u64;
                let range = parse_range(request.headers(), len);
                let code = if range.partial {
                    StatusCode::PARTIAL_CONTENT
                } else {
                    StatusCode::OK
                };
                bytes_response(code, &range, len, "application/javascript", bytes, None)
            }
            Err(e) => {
                log::warn!("AllFonts.js unavailable ({e}) — editor will run with fallback fonts");
                empty(StatusCode::NOT_FOUND)
            }
        };
    }

    // font files
    if let Some(rest) = path.strip_prefix("fonts/") {
        let Some((root, rel)) = rest.split_once('/') else {
            return empty(StatusCode::NOT_FOUND);
        };
        let Some(disk_root) = allfonts::font_root_disk(root) else {
            return empty(StatusCode::FORBIDDEN);
        };
        let Some(full) = safe_join(&disk_root, rel) else {
            return empty(StatusCode::FORBIDDEN);
        };
        return serve_file(&full, &request, None);
    }

    // engine assets
    let Some(rel) = path.strip_prefix("editors/") else {
        return empty(StatusCode::NOT_FOUND);
    };
    let Some(full) = safe_join(&engines.join("editors"), rel) else {
        return empty(StatusCode::FORBIDDEN);
    };
    serve_file(&full, &request, None)
}

pub fn handle_file<R: tauri::Runtime>(
    ctx: tauri::UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    let path = request.uri().path().trim_start_matches('/');
    let path = percent_decode(path);
    log::debug!("file GET {path}");
    let Some(rest) = path.strip_prefix("file/") else {
        return empty(StatusCode::NOT_FOUND);
    };
    let Some((id_s, _name)) = rest.split_once('/') else {
        return empty(StatusCode::NOT_FOUND);
    };
    let Ok(id) = id_s.parse::<u32>() else {
        return empty(StatusCode::NOT_FOUND);
    };
    let Some(disk) = ctx.app_handle().state::<FileRegistry>().get(id) else {
        log::warn!("protocol file 404: unknown fileId {id}");
        return empty(StatusCode::NOT_FOUND);
    };
    log::info!("serving document fileId={id} → {}", disk.display());
    // Suggest the original filename (inline view; the engine fetches by URL).
    let disposition = disk.file_name().map(|n| {
        format!(
            "inline; filename*=UTF-8''{}",
            allfonts::encode_uri_component(&n.to_string_lossy())
        )
    });
    serve_file(&disk, &request, disposition)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_roundtrip() {
        // cross-module contract (T-054): the encoder lives in
        // protocol::allfonts, the decoder here — they must roundtrip.
        let enc = allfonts::encode_uri_component("Avenir Next Condensed.ttc");
        assert_eq!(enc, "Avenir%20Next%20Condensed.ttc");
        assert_eq!(percent_decode(&enc), "Avenir Next Condensed.ttc");
    }

    #[test]
    fn range_parse_basic() {
        let mut headers = tauri::http::HeaderMap::new();
        headers.insert(header::RANGE, "bytes=10-19".parse().unwrap());
        let r = parse_range(&headers, 100);
        assert!(r.partial && r.start == 10 && r.end == 19);

        headers.insert(header::RANGE, "bytes=-5".parse().unwrap());
        let r = parse_range(&headers, 100);
        assert!(r.partial && r.start == 95 && r.end == 99);

        let r = parse_range(&tauri::http::HeaderMap::new(), 100);
        assert!(!r.partial);
    }

    #[test]
    fn serve_file_full_body_has_no_nul_padding() {
        // Regression: non-Range reads used a zero-filled buffer + read_to_end,
        // doubling the body with NUL padding (broke api.js with SyntaxError).
        let path = std::env::temp_dir().join("nanooffice-test-nul.txt");
        std::fs::write(&path, b"hello nanooffice").expect("write temp");
        let request = Request::builder().body(Vec::new()).expect("request");
        let response = serve_file(&path, &request, None);
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.body(), b"hello nanooffice");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn serve_file_honours_range() {
        let path = std::env::temp_dir().join("nanooffice-test-range.txt");
        std::fs::write(&path, b"0123456789").expect("write temp");
        let request = Request::builder()
            .header(header::RANGE, "bytes=2-5")
            .body(Vec::new())
            .expect("request");
        let response = serve_file(&path, &request, None);
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(response.body(), b"2345");
        let _ = std::fs::remove_file(&path);
    }
}
