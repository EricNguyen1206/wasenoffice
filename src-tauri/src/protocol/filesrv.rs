//! Loopback HTTP server serving the ENGINE and its documents from one origin.
//!
//! Why it exists: the engine classifies document URLs by string scheme
//! (upstream/sdkjs/common/editorscommon.js:3088) and rejects every custom
//! scheme as `Invalid`, so a `wasenoffice-file://…` document URL is never
//! fetched. Serving everything (engine assets under `/editors/`, fonts under
//! `/fonts/`, documents under `/file/<id>/`) from `http://127.0.0.1:<port>`
//! gives the editor iframe and its document one origin: no CORS, no mixed
//! content, and a scheme the engine accepts — while disk paths stay Rust-side
//! (URLs carry opaque fileIds only). Range requests supported.
//! Architecture deviation recorded in docs/reference.map.md — revisit in PH-1.

use std::io::{BufRead, BufReader, Read};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;

use super::FileRegistry;

pub const DEFAULT_PORT: u16 = 14321;

/// Largest `POST /invoke` body accepted (keymap overrides are ~KBs; a paste of
/// a whole document through `wasen_light_edit` stays far below this).
const MAX_INVOKE_BODY: usize = 1 << 20;

static BOUND_PORT: std::sync::OnceLock<u16> = std::sync::OnceLock::new();

/// Port the loopback file server actually bound (DEFAULT_PORT when free).
pub fn bound_port() -> u16 {
    *BOUND_PORT.get().unwrap_or(&DEFAULT_PORT)
}

/// Start serving `registry` on 127.0.0.1. Returns the port actually bound.
/// `bridge` = the app handle when web dev mode is on (T-068): the server then
/// also answers `POST /invoke/<cmd>` (`crate::web_bridge`); `None` = 404.
pub fn spawn(registry: Arc<FileRegistry>, bridge: Option<tauri::AppHandle>) -> Result<u16, String> {
    let listener = TcpListener::bind(("127.0.0.1", DEFAULT_PORT))
        .or_else(|_| TcpListener::bind(("127.0.0.1", 0)))
        .map_err(|e| format!("cannot bind loopback file server: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let _ = BOUND_PORT.set(port);
    log::info!("loopback file server on 127.0.0.1:{port}");
    if bridge.is_some() {
        log::info!(
            "web bridge: listening (origins: {})",
            crate::web_bridge::ALLOWED_ORIGINS.join(", ")
        );
    }
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let registry = Arc::clone(&registry);
                    let bridge = bridge.clone();
                    std::thread::spawn(move || handle(stream, registry, bridge));
                }
                Err(e) => log::warn!("file server accept error: {e}"),
            }
        }
    });
    Ok(port)
}

fn handle(mut stream: TcpStream, registry: Arc<FileRegistry>, bridge: Option<tauri::AppHandle>) {
    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("");
    log::info!("file server ← {method} {target}");
    if method == "OPTIONS" {
        // CORS preflight (the engine's XHR carries non-simple headers; the web
        // bridge POSTs JSON with its window-label header).
        let resp = "HTTP/1.1 204 No Content\r\n\
            Access-Control-Allow-Origin: *\r\n\
            Access-Control-Allow-Methods: GET, HEAD, POST, OPTIONS\r\n\
            Access-Control-Allow-Headers: Range, Authorization, Content-Type, X-Wasen-Window\r\n\
            Access-Control-Max-Age: 600\r\n\
            Content-Length: 0\r\n\r\n";
        let _ = std::io::Write::write_all(&mut stream, resp.as_bytes());
        return;
    }
    if method == "POST" {
        let cmd = target
            .split('?')
            .next()
            .and_then(|p| p.strip_prefix("/invoke/"))
            .map(super::percent_decode)
            .filter(|c| !c.is_empty());
        match (cmd, bridge) {
            (Some(cmd), Some(app)) => handle_invoke(&mut reader, &app, &cmd),
            // web dev mode OFF (the default) — the route does not exist
            (Some(_), None) => respond(&mut stream, 404, &[], &[], false),
            (None, _) => respond(&mut stream, 405, &[], &[], false),
        }
        return;
    }
    if method != "GET" && method != "HEAD" {
        respond(&mut stream, 405, &[], &[], false);
        return;
    }
    let path = target.split('?').next().unwrap_or(target);
    let path = super::percent_decode(path);

    let range_header: Option<String> = reader
        .lines()
        .map_while(Result::ok)
        .take_while(|l| !l.is_empty())
        .find_map(|l| {
            l.strip_prefix("Range: ")
                .or_else(|| l.strip_prefix("range: "))
                .map(str::to_owned)
        });

    // engine assets + fonts: same routes as the wasenoffice-engine:// handler
    let engine_path = path.trim_start_matches('/').to_string();
    if engine_path == "editors/sdkjs/common/AllFonts.js" {
        match super::allfonts::engine_flavor_js(bound_port()) {
            Ok(bytes) => {
                respond_bytes(&mut stream, "application/javascript", bytes, false);
            }
            Err(e) => {
                log::warn!("AllFonts.js unavailable ({e}) — editor will run with fallback fonts");
                respond(&mut stream, 404, &[], &[], false);
            }
        }
        return;
    }
    if let Some(rest) = engine_path.strip_prefix("fonts/") {
        match resolve_font_path(rest) {
            Some(full) => {
                // T-A12 additive log (log contract untouched): shows which
                // disk root a /fonts/<root>/… request is served from.
                log::info!("font serve: /fonts/{rest} from {}", full.display());
                serve(
                    &mut stream,
                    &full,
                    range_header.as_deref(),
                    method == "HEAD",
                );
            }
            None => respond(&mut stream, 403, &[], &[], false),
        }
        return;
    }
    if let Some(rest) = engine_path.strip_prefix("editors/") {
        let mut full = super::allfonts::engines_root()
            .and_then(|root| super::safe_join(&root.join("editors"), rest));
        // Emscripten glue loaded via LoadJS eval loses scriptDirectory and
        // resolves wasm payloads relative to the editor page; when the
        // requested location does not exist, redirect to the canonical
        // sdkjs engine payload by basename.
        if full.as_ref().is_none_or(|p| !p.is_file()) {
            full = resolve_engine_payload(rest).or(full);
        }
        match full {
            Some(full) => serve(
                &mut stream,
                &full,
                range_header.as_deref(),
                method == "HEAD",
            ),
            None => respond(&mut stream, 404, &[], &[], false),
        }
        return;
    }

    // T-V3 media route: OPC parts of a REGISTERED document, keyed by fileId
    // (same contract as /file/<id>/ — no disk paths travel through URLs). The
    // part name is looked up EXACTLY in the zip's entry table
    // (`try_part_bytes`), so a traversal name can never resolve to a disk
    // path; anything that merely LOOKS like traversal is refused with 403
    // before the lookup.
    if let Some(rest) = path.strip_prefix("/media/") {
        let Some((id_s, part)) = rest.split_once('/') else {
            respond(&mut stream, 404, &[], &[], false);
            return;
        };
        let Ok(id) = id_s.parse::<u32>() else {
            respond(&mut stream, 404, &[], &[], false);
            return;
        };
        let Some(disk) = registry.get(id) else {
            respond(&mut stream, 404, &[], &[], false);
            return;
        };
        if part.split('/').any(|seg| seg == "..") || part.contains('\\') {
            log::warn!("file server 403: media part {part:?} looks like traversal");
            respond(&mut stream, 403, &[], &[], false);
            return;
        }
        let Ok(zip_bytes) = std::fs::read(&disk) else {
            respond(&mut stream, 404, &[], &[], false);
            return;
        };
        match wasenoffice_ooxml::try_part_bytes(&zip_bytes, part) {
            Some(bytes) => {
                log::info!(
                    "media serve: fileId {id} part {part} ({} bytes)",
                    bytes.len()
                );
                respond_bytes(&mut stream, super::mime_for(part), bytes, method == "HEAD");
            }
            None => respond(&mut stream, 404, &[], &[], false),
        }
        return;
    }

    // T-067 D2: open-delivery streaming route — the bytes of the DELIVERED
    // artifact (Editor.bin for convert-flavor, copied original for pdf) that
    // the ENGINE frame fetches itself (shim delivery.js) instead of receiving
    // them through the IPC/base64 lane (DD open-streaming D1). Same shape as
    // /media: opaque fileId in the URL; the name must MATCH the artifact name
    // that format's open flavor is expected to deliver — anything else
    // (traversal included) is refused with 403 before any disk read.
    if let Some(rest) = path.strip_prefix("/delivery/") {
        let Some((id_s, name)) = rest.split_once('/') else {
            respond(&mut stream, 404, &[], &[], false);
            return;
        };
        let Ok(id) = id_s.parse::<u32>() else {
            respond(&mut stream, 404, &[], &[], false);
            return;
        };
        let Some(disk) = registry.get(id) else {
            log::warn!("file server 404: unknown fileId {id}");
            respond(&mut stream, 404, &[], &[], false);
            return;
        };
        if name.split('/').any(|seg| seg == "..") || name.contains('\\') {
            log::warn!("file server 403: delivery name {name:?} looks like traversal");
            respond(&mut stream, 403, &[], &[], false);
            return;
        }
        let ext = disk
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let flavor = crate::formats::by_ext(&ext)
            .map(|f| f.open)
            .unwrap_or(crate::formats::OpenFlavor::Convert);
        let work = crate::formats::DocWorkdir::new(id);
        match work.delivered_name(flavor, &disk) {
            Ok(expected) if expected == name => serve(
                &mut stream,
                &work.root().join(name),
                range_header.as_deref(),
                method == "HEAD",
            ),
            other => {
                log::warn!(
                    "file server 403: delivery name {name:?} != expected {:?}",
                    other.ok()
                );
                respond(&mut stream, 403, &[], &[], false);
            }
        }
        return;
    }

    let Some(rest) = path.strip_prefix("/file/") else {
        respond(&mut stream, 404, &[], &[], false);
        return;
    };
    let Some((id_s, _name)) = rest.split_once('/') else {
        respond(&mut stream, 404, &[], &[], false);
        return;
    };
    let Ok(id) = id_s.parse::<u32>() else {
        respond(&mut stream, 404, &[], &[], false);
        return;
    };
    let Some(disk) = registry.get(id) else {
        log::warn!("file server 404: unknown fileId {id}");
        respond(&mut stream, 404, &[], &[], false);
        return;
    };

    serve(
        &mut stream,
        &disk,
        range_header.as_deref(),
        method == "HEAD",
    );
}

fn serve(stream: &mut TcpStream, disk: &PathBuf, range: Option<&str>, head_only: bool) {
    let Ok(mut file) = std::fs::File::open(disk) else {
        respond(stream, 404, &[], &[], false);
        return;
    };
    let total = file.metadata().map(|m| m.len()).unwrap_or(0);
    let mime = super::mime_for(&disk.to_string_lossy());
    let fname = disk
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "document".into());

    // bytes=start-end | bytes=-suffix | none — parsed by the ONE tested
    // parser (mod.rs::parse_range_header, T-054; the inline re-implementation
    // that used to live here had no tests and mishandled 0-byte bodies).
    let range = super::parse_range_header(range, total);

    let length = if total == 0 {
        0
    } else {
        (range.end - range.start + 1) as usize
    };

    let mut headers = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {mime}\r\nAccept-Ranges: bytes\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Content-Disposition: inline; filename*=UTF-8''{}\r\n",
        if range.partial { 206 } else { 200 },
        if range.partial {
            "Partial Content"
        } else {
            "OK"
        },
        super::allfonts::encode_uri_component(&fname),
    );
    if range.partial {
        headers.push_str(&format!(
            "Content-Range: bytes {}-{}/{}\r\n",
            range.start, range.end, total
        ));
    }
    headers.push_str(&format!("Content-Length: {}\r\n\r\n", length));

    let _ = std::io::Write::write_all(stream, headers.as_bytes());
    if head_only || length == 0 {
        return;
    }
    // T-067 D3: stream in chunks — the whole-file `vec![0; len]` buffer here
    // made the server thread hold N bytes for every delivery (the multiplier
    // that thrashed near-open RAM on big documents). Headers above are
    // unchanged, so the wire output is identical.
    use std::io::{Read, Seek};
    if file.seek(std::io::SeekFrom::Start(range.start)).is_err() {
        return;
    }
    let mut buf = [0u8; 64 * 1024];
    let mut remaining = length as u64;
    while remaining > 0 {
        let want = remaining.min(buf.len() as u64) as usize;
        match file.read(&mut buf[..want]) {
            Ok(0) => break,
            Ok(n) => {
                if std::io::Write::write_all(stream, &buf[..n]).is_err() {
                    return;
                }
                remaining -= n as u64;
            }
            Err(_) => break,
        }
    }
}

/// `POST /invoke/<cmd>` (web dev mode, T-068): headers → origin gate → JSON
/// body → `web_bridge::dispatch`. App-level failures travel as HTTP 200
/// `{"ok":false,"error"}` so the browser-side polyfill rejects exactly like
/// Tauri's `invoke`; only transport problems (origin, size, syntax) use 4xx.
/// Writes through the reader's own `&TcpStream` (std: `Write for &TcpStream`).
fn handle_invoke(reader: &mut BufReader<&TcpStream>, app: &tauri::AppHandle, cmd: &str) {
    let mut out: &TcpStream = reader.get_ref();
    let (mut origin, mut label, mut len) = (None, None, 0usize);
    for line in reader
        .by_ref()
        .lines()
        .map_while(Result::ok)
        .take_while(|l| !l.is_empty())
    {
        if let Some((k, v)) = line.split_once(':') {
            let v = v.trim();
            match k.to_ascii_lowercase().as_str() {
                "origin" => origin = Some(v.to_string()),
                "x-wasen-window" => label = Some(v.to_string()),
                "content-length" => len = v.parse().unwrap_or(0),
                _ => {}
            }
        }
    }
    if !crate::web_bridge::origin_allowed(origin.as_deref()) {
        log::warn!("web bridge: refused origin {origin:?} for {cmd}");
        respond(&mut out, 403, &[], &[], false);
        return;
    }
    if len > MAX_INVOKE_BODY {
        respond(&mut out, 413, &[], &[], false);
        return;
    }
    let mut body = vec![0u8; len];
    if reader.read_exact(&mut body).is_err() {
        respond(&mut out, 400, &[], &[], false);
        return;
    }
    let args: serde_json::Value = if body.is_empty() {
        serde_json::json!({})
    } else {
        match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(_) => {
                respond(&mut out, 400, &[], &[], false);
                return;
            }
        }
    };
    let label = label.unwrap_or_else(|| "web".to_string());
    let reply = match crate::web_bridge::dispatch(app, &label, cmd, args) {
        Ok(result) => serde_json::json!({ "ok": true, "result": result }),
        Err(error) => serde_json::json!({ "ok": false, "error": error }),
    };
    let body = reply.to_string();
    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\n\
         Access-Control-Allow-Origin: {}\r\nContent-Length: {}\r\n\r\n",
        origin.unwrap_or_default(),
        body.len()
    );
    let _ = std::io::Write::write_all(&mut out, resp.as_bytes());
    let _ = std::io::Write::write_all(&mut out, body.as_bytes());
}

fn respond<W: std::io::Write>(
    stream: &mut W,
    code: u16,
    _headers: &[(&str, &str)],
    _body: &[u8],
    head: bool,
) {
    let status = match code {
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        _ => "OK",
    };
    let resp = format!("HTTP/1.1 {code} {status}\r\nContent-Length: 0\r\n\r\n");
    let _ = std::io::Write::write_all(stream, resp.as_bytes());
    let _ = head;
}

/// Known engine payloads (wasm/wasm-adjacent) resolved by basename from the
/// canonical sdkjs locations, regardless of the requesting page directory.
fn resolve_engine_payload(rest: &str) -> Option<PathBuf> {
    let basename = rest.rsplit('/').next()?;
    let dir = match basename {
        "fonts.wasm" => "sdkjs/common/libfont/engine/",
        "zlib.wasm" => "sdkjs/common/zlib/engine/",
        "engine.wasm" => "sdkjs/common/hash/hash/engine/",
        "drawingfile.wasm" => "sdkjs/pdf/src/engine/",
        "spell.wasm" => "sdkjs/common/spell/spell/",
        _ => return None,
    };
    let engines = super::allfonts::engines_root()?;
    super::safe_join(&engines.join("editors"), &format!("{dir}{basename}"))
}

/// `app/<enc>` | `sys/<enc>` → whitelisted absolute font path.
/// Root table: `protocol::allfonts::font_root_disk` (the producer, T-054).
fn resolve_font_path(rest: &str) -> Option<PathBuf> {
    let (root, rel) = rest.split_once('/')?;
    let disk_root = super::allfonts::font_root_disk(root)?;
    super::safe_join(&disk_root, rel)
}

fn respond_bytes(stream: &mut TcpStream, mime: &str, body: Vec<u8>, head: bool) {
    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nAccept-Ranges: bytes\r\n\
         Access-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    let _ = std::io::Write::write_all(stream, resp.as_bytes());
    if !head {
        let _ = std::io::Write::write_all(stream, &body);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::sync::Arc;

    /// The T-A26 media fixture: real multi-pixel PNGs, one body + one header
    /// part (bytes ≠ the writer's 1×1 white placeholder by construction).
    fn media_fixture() -> &'static str {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/crates/wasenoffice-ooxml/tests/fixtures/wasenoffice-media-fixture.docx"
        )
    }

    /// One shared server per test binary; tests register their own files.
    /// Bridge OFF — the production default; the ON path is gate 21's job
    /// (needs a running app handle).
    fn server() -> (u16, Arc<FileRegistry>) {
        static SERVER: std::sync::OnceLock<(u16, Arc<FileRegistry>)> = std::sync::OnceLock::new();
        SERVER
            .get_or_init(|| {
                let registry = Arc::new(FileRegistry::default());
                let port = spawn(Arc::clone(&registry), None).expect("bind test filesrv");
                (port, registry)
            })
            .clone()
    }

    fn request(port: u16, raw: &str) -> (u16, Vec<u8>) {
        let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect");
        s.write_all(raw.as_bytes()).expect("write request");
        let mut buf = Vec::new();
        s.read_to_end(&mut buf).expect("read response");
        let head = String::from_utf8_lossy(&buf);
        let status: u16 = head
            .lines()
            .next()
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|c| c.parse().ok())
            .unwrap_or(0);
        let body = match buf.windows(4).position(|w| w == b"\r\n\r\n") {
            Some(i) => buf[i + 4..].to_vec(),
            None => Vec::new(),
        };
        (status, body)
    }

    fn get(port: u16, target: &str) -> (u16, Vec<u8>) {
        request(port, &format!("GET {target} HTTP/1.1\r\nHost: t\r\n\r\n"))
    }

    /// T-068 release safety: without `WASEN_OFFICE_WEB_BRIDGE` the invoke
    /// route does not exist (404) even from an allowed origin, and POST
    /// anywhere else stays 405 — the GET surface is untouched.
    #[test]
    fn invoke_route_is_absent_when_bridge_off() {
        let (port, _) = server();
        let body = r#"{"path":"/x.docx"}"#;
        let (status, _) = request(
            port,
            &format!(
                "POST /invoke/recents_list HTTP/1.1\r\nHost: t\r\nOrigin: http://127.0.0.1:1420\r\n\
                 X-Wasen-Window: web-test\r\nContent-Type: application/json\r\n\
                 Content-Length: {}\r\n\r\n{body}",
                body.len()
            ),
        );
        assert_eq!(status, 404);
        let (status, _) = request(port, "POST /media/1/x HTTP/1.1\r\nHost: t\r\n\r\n");
        assert_eq!(status, 405);
        let (status, _) = request(port, "POST /invoke/ HTTP/1.1\r\nHost: t\r\n\r\n");
        assert_eq!(status, 405, "empty command name is not a route");
    }

    #[test]
    fn media_serves_fixture_bytes_byte_exact() {
        let (port, registry) = server();
        let id = registry
            .register("test", std::path::Path::new(media_fixture()))
            .unwrap();
        let raw = std::fs::read(media_fixture()).unwrap();
        let expected = wasenoffice_ooxml::try_part_bytes(&raw, "word/media/image1.png").unwrap();
        let other = wasenoffice_ooxml::try_part_bytes(&raw, "word/media/image2.png").unwrap();
        assert!(
            !expected.is_empty() && expected != other,
            "fixture parts indistinct"
        );

        let (status, body) = get(port, &format!("/media/{id}/word/media/image1.png"));
        assert_eq!(status, 200);
        assert_eq!(
            body, expected,
            "media route must serve the OPC part byte-exact"
        );

        // a part inside the registered file but never declared in rels is
        // still a part of THAT file — exact-name lookup, no rels check (the
        // viewer resolves rels before ever building a URL)
        let (status, body) = get(port, &format!("/media/{id}/word/document.xml"));
        assert_eq!(status, 200);
        assert!(!body.is_empty());
    }

    #[test]
    fn delivery_route_serves_only_the_expected_artifact() {
        let (port, registry) = server();
        let id = registry
            .register("test", std::path::Path::new(media_fixture()))
            .unwrap();
        // docx → Convert flavor → the delivered artifact is exactly Editor.bin
        let work = crate::formats::DocWorkdir::new(id);
        std::fs::create_dir_all(work.root()).unwrap();
        let body: &[u8] = b"FAKE-DOCY-BIN";
        std::fs::write(work.editor_bin(), body).unwrap();

        // exact artifact name → byte-exact body
        let (status, got) = get(port, &format!("/delivery/{id}/Editor.bin"));
        assert_eq!(status, 200);
        assert_eq!(
            got,
            body.to_vec(),
            "delivery must stream Editor.bin byte-exact"
        );

        // mismatched name (wrong artifact/flavor) → 403 before any disk read
        let (status, _) = get(port, &format!("/delivery/{id}/whatever.pdf"));
        assert_eq!(
            status, 403,
            "flavor-mismatched delivery name must be refused"
        );
        // traversal-literal and percent-encoded → 403
        let (status, _) = get(port, &format!("/delivery/{id}/../secret"));
        assert_eq!(status, 403);
        // unknown fileId → 404
        let (status, _) = get(port, "/delivery/777777/Editor.bin");
        assert_eq!(status, 404);
        // no name in the URL → 404
        let (status, _) = get(port, &format!("/delivery/{id}"));
        assert_eq!(status, 404);

        std::fs::remove_dir_all(work.root()).ok();
    }

    #[test]
    fn media_rejects_traversal_and_unknown_targets() {
        let (port, registry) = server();
        let id = registry
            .register("test", std::path::Path::new(media_fixture()))
            .unwrap();

        let (status, _) = get(port, &format!("/media/{id}/../secret"));
        assert_eq!(status, 403, "literal ../ must be refused");
        let (status, _) = get(port, &format!("/media/{id}/word/media/%2E%2E/evil"));
        assert_eq!(status, 403, "percent-encoded ../ must be refused");
        let (status, _) = get(port, &format!("/media/{id}/word\\media\\image1.png"));
        assert_eq!(status, 403, "backslash separator must be refused");

        let (status, _) = get(port, &format!("/media/{id}/word/media/absent.png"));
        assert_eq!(status, 404, "unknown part of a known file");
        let (status, _) = get(port, "/media/424242/word/media/image1.png");
        assert_eq!(status, 404, "unknown fileId");
        let (status, _) = get(port, &format!("/media/{id}"));
        assert_eq!(status, 404, "no part in the URL");
    }
}
