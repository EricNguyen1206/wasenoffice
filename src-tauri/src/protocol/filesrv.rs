//! Loopback HTTP server serving the ENGINE and its documents from one origin.
//!
//! Why it exists: the engine classifies document URLs by string scheme
//! (upstream/sdkjs/common/editorscommon.js:3088) and rejects every custom
//! scheme as `Invalid`, so a `nanooffice-file://…` document URL is never
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

static BOUND_PORT: std::sync::OnceLock<u16> = std::sync::OnceLock::new();

/// Port the loopback file server actually bound (DEFAULT_PORT when free).
pub fn bound_port() -> u16 {
    *BOUND_PORT.get().unwrap_or(&DEFAULT_PORT)
}

/// Start serving `registry` on 127.0.0.1. Returns the port actually bound.
pub fn spawn(registry: Arc<FileRegistry>) -> Result<u16, String> {
    let listener = TcpListener::bind(("127.0.0.1", DEFAULT_PORT))
        .or_else(|_| TcpListener::bind(("127.0.0.1", 0)))
        .map_err(|e| format!("cannot bind loopback file server: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let _ = BOUND_PORT.set(port);
    log::info!("loopback file server on 127.0.0.1:{port}");
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let registry = Arc::clone(&registry);
                    std::thread::spawn(move || handle(stream, registry));
                }
                Err(e) => log::warn!("file server accept error: {e}"),
            }
        }
    });
    Ok(port)
}

fn handle(mut stream: TcpStream, registry: Arc<FileRegistry>) {
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
        // CORS preflight (the engine's XHR carries non-simple headers).
        let resp = "HTTP/1.1 204 No Content\r\n\
            Access-Control-Allow-Origin: *\r\n\
            Access-Control-Allow-Methods: GET, HEAD, OPTIONS\r\n\
            Access-Control-Allow-Headers: Range, Authorization, Content-Type\r\n\
            Access-Control-Max-Age: 600\r\n\
            Content-Length: 0\r\n\r\n";
        let _ = std::io::Write::write_all(&mut stream, resp.as_bytes());
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

    // engine assets + fonts: same routes as the nanooffice-engine:// handler
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
    let mut body = vec![0u8; length];
    if length > 0 {
        use std::io::Seek;
        if file.seek(std::io::SeekFrom::Start(range.start)).is_ok() {
            let _ = file.read_exact(&mut body);
        }
    }

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
    headers.push_str(&format!("Content-Length: {}\r\n\r\n", body.len()));

    let _ = std::io::Write::write_all(stream, headers.as_bytes());
    if !head_only {
        let _ = std::io::Write::write_all(stream, &body);
    }
}

fn respond(stream: &mut TcpStream, code: u16, _headers: &[(&str, &str)], _body: &[u8], head: bool) {
    let status = match code {
        404 => "Not Found",
        405 => "Method Not Allowed",
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

fn respond_bytes(mut stream: &mut TcpStream, mime: &str, body: Vec<u8>, head: bool) {
    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nAccept-Ranges: bytes\r\n\
         Access-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    let _ = std::io::Write::write_all(&mut stream, resp.as_bytes());
    if !head {
        let _ = std::io::Write::write_all(&mut stream, &body);
    }
}
