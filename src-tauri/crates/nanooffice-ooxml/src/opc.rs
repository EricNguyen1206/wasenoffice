//! OPC package layer (ECMA-376 Part 2 — Open Packaging Conventions).
//!
//! A docx is a zip; parts are named zip entries. `[Content_Types].xml`
//! declares part types (Defaults by extension, Overrides by part name);
//! relationships live in `<dir>/_rels/<file>.rels` parts. Part names and
//! relationship targets are resolved per the OPC naming rules — the main
//! document part is looked up, never hard-coded.

use std::io::Read as _;

use quick_xml::events::Event;

use crate::{xml_err, ParseError};

/// Content type of the WordprocessingML main document part.
pub const CT_MAIN_DOCUMENT: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";

/// Content types of the WordprocessingML accessory parts the model covers
/// (Part 2 §10.1.2.1.x media-type names, Part 1 part definitions).
pub const CT_STYLES: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml";
pub const CT_NUMBERING: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml";
pub const CT_HEADER: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml";
pub const CT_FOOTER: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml";
/// Content type of relationships parts (Part 2, package relationship part).
pub const CT_RELS: &str = "application/vnd.openxmlformats-package.relationships+xml";

/// Relationship types (Part 2 / §17.16, officeDocument/2006/relationships/*).
pub const RT_OFFICE_DOCUMENT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
pub const RT_STYLES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles";
pub const RT_NUMBERING: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering";
pub const RT_IMAGE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
pub const RT_HEADER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header";
pub const RT_FOOTER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer";
pub const RT_HYPERLINK: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";
pub const RT_SETTINGS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings";
pub const RT_THEME: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme";
/// Package-level metadata relationships (`_rels/.rels`).
pub const RT_CORE_PROPERTIES: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties";
pub const RT_EXTENDED_PROPERTIES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties";

/// One `Relationship` element of a relationships part.
#[derive(Debug, Clone)]
pub struct Rel {
    pub rel_type: String,
    pub target: String,
    /// `TargetMode="External"` — the target is a URI, not a package part.
    pub external: bool,
}

/// Read one part's bytes by its package path (e.g. `word/document.xml`).
pub fn part_bytes(zip_bytes: &[u8], name: &str) -> Result<Vec<u8>, ParseError> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes))
        .map_err(|e| ParseError(format!("zip: {e}")))?;
    let mut part = archive
        .by_name(name)
        .map_err(|e| ParseError(format!("part {name}: {e}")))?;
    let mut out = Vec::with_capacity(part.size() as usize);
    part.read_to_end(&mut out)
        .map_err(|e| ParseError(format!("read part {name}: {e}")))?;
    Ok(out)
}

/// Like `part_bytes`, but a missing part is `None` (a part may legally have
/// no relationships part). Corrupt zips still error.
pub fn try_part_bytes(zip_bytes: &[u8], name: &str) -> Option<Vec<u8>> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).ok()?;
    let mut part = archive.by_name(name).ok()?;
    let mut out = Vec::with_capacity(part.size() as usize);
    part.read_to_end(&mut out).ok()?;
    Some(out)
}

/// `[Content_Types].xml` (Part 2): Defaults by extension + Overrides by part
/// name. Part names are stored without the leading slash (targets resolve to
/// that form too).
#[derive(Debug, Default)]
pub struct ContentTypes {
    defaults: std::collections::BTreeMap<String, String>,
    overrides: std::collections::BTreeMap<String, String>,
}

impl ContentTypes {
    pub fn parse(xml: &str) -> Result<Self, ParseError> {
        let mut reader = quick_xml::Reader::from_str(xml);
        let mut buf = Vec::new();
        let mut ct = ContentTypes::default();
        loop {
            buf.clear();
            match reader.read_event_into(&mut buf).map_err(xml_err)? {
                Event::Start(e) | Event::Empty(e) => match e.name().as_ref() {
                    b"Default" => {
                        let ext = attr(&e, b"Extension")?
                            .ok_or_else(|| ParseError("Default missing Extension".into()))?;
                        let ty = attr(&e, b"ContentType")?
                            .ok_or_else(|| ParseError("Default missing ContentType".into()))?;
                        ct.defaults.insert(ext, ty);
                    }
                    b"Override" => {
                        let part = attr(&e, b"PartName")?
                            .ok_or_else(|| ParseError("Override missing PartName".into()))?;
                        let ty = attr(&e, b"ContentType")?
                            .ok_or_else(|| ParseError("Override missing ContentType".into()))?;
                        ct.overrides
                            .insert(part.trim_start_matches('/').to_string(), ty);
                    }
                    _ => {}
                },
                Event::Eof => return Ok(ct),
                _ => {}
            }
        }
    }

    /// Content type of a part: Override wins, then Default by extension.
    pub fn get(&self, part: &str) -> Option<&str> {
        if let Some(t) = self.overrides.get(part) {
            return Some(t.as_str());
        }
        let ext = part.rsplit_once('.')?.1;
        self.defaults.get(ext).map(|s| s.as_str())
    }
}

/// Parse a relationships part into `(Id, Rel)` pairs in file order.
pub fn parse_rels(xml: &str) -> Result<Vec<(String, Rel)>, ParseError> {
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut rels = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf).map_err(xml_err)? {
            Event::Start(e) | Event::Empty(e) => {
                if e.name().as_ref() == b"Relationship" {
                    let id = attr(&e, b"Id")?
                        .ok_or_else(|| ParseError("Relationship missing Id".into()))?;
                    let rel_type = attr(&e, b"Type")?
                        .ok_or_else(|| ParseError("Relationship missing Type".into()))?;
                    let target = attr(&e, b"Target")?
                        .ok_or_else(|| ParseError("Relationship missing Target".into()))?;
                    let external = attr(&e, b"TargetMode")?.as_deref() == Some("External");
                    rels.push((
                        id,
                        Rel {
                            rel_type,
                            target,
                            external,
                        },
                    ));
                }
            }
            Event::Eof => return Ok(rels),
            _ => {}
        }
    }
}

/// Resolve a relationship target to a package part name relative to the
/// source part's directory. Leading-slash targets are package-rooted;
/// `..` segments are refused rather than second-guessed.
pub fn resolve_target(base_dir: &str, target: &str) -> Result<String, ParseError> {
    if target.contains("..") {
        return Err(ParseError(format!(
            "relationship target {target:?} contains `..` — refusing to resolve"
        )));
    }
    if let Some(rest) = target.strip_prefix('/') {
        return Ok(rest.to_string());
    }
    if base_dir.is_empty() {
        Ok(target.to_string())
    } else {
        Ok(format!("{base_dir}/{target}"))
    }
}

/// One attribute by its prefixed key, unescaped.
pub(crate) fn attr(
    e: &quick_xml::events::BytesStart<'_>,
    key: &[u8],
) -> Result<Option<String>, ParseError> {
    for a in e.attributes() {
        let a = a.map_err(|e| ParseError(format!("xml attr: {e}")))?;
        if a.key.as_ref() == key {
            return a
                .unescape_value()
                .map(|v| v.into_owned())
                .map(Some)
                .map_err(|e| ParseError(format!("xml attr value: {e}")));
        }
    }
    Ok(None)
}

// --- relationships plumbing (review #6 C1: ONE home for the "main part is
// --- data, never hard-coded" invariant and the rels/path helpers) ---------

/// UTF-8 decode of a package part with the part name in the error.
pub(crate) fn utf8(bytes: Vec<u8>, what: &str) -> Result<String, ParseError> {
    String::from_utf8(bytes).map_err(|e| ParseError(format!("utf-8 {what}: {e}")))
}

/// All relationships of one part, in relationship-file order.
pub(crate) type RelsList = Vec<(String, Rel)>;

/// All relationships of one part (`<dir>/_rels/<file>.rels`); a missing rels
/// stream is legal — an empty list, not an error.
pub(crate) fn load_rels(zip_bytes: &[u8], dir: &str, file: &str) -> Result<RelsList, ParseError> {
    let rels_part = if dir.is_empty() {
        format!("_rels/{file}.rels")
    } else {
        format!("{dir}/_rels/{file}.rels")
    };
    match try_part_bytes(zip_bytes, &rels_part) {
        Some(bytes) => parse_rels(&utf8(bytes, &rels_part)?),
        None => Ok(Vec::new()), // a part may legally carry no relationships
    }
}

/// Target of the first relationship of the given type, resolved against the
/// source part's directory.
pub(crate) fn find_target(
    rels: &RelsList,
    rel_type: &str,
    dir: &str,
) -> Result<Option<String>, ParseError> {
    rels.iter()
        .find(|(_, r)| r.rel_type == rel_type)
        .map(|(_, r)| resolve_target(dir, &r.target))
        .transpose()
}

/// Directory part of a package path (`word/media/x.png` → `word`).
pub(crate) fn part_dir(part: &str) -> String {
    part.rsplit_once('/')
        .map(|(d, _)| d.to_string())
        .unwrap_or_default()
}

/// File name of a package path (`word/header1.xml` → `header1.xml`).
pub(crate) fn file_name(part: &str) -> String {
    part.rsplit_once('/')
        .map(|(_, f)| f.to_string())
        .unwrap_or_else(|| part.to_string())
}

/// Main document part name of a package — looked up through the
/// PACKAGE-level relationships. `_rels/.rels` is read directly here:
/// `load_rels` is for a PART's rels and would look for `_rels/.rels.rels`
/// (the T-A31 bug class, now structurally impossible to re-import).
pub(crate) fn main_document_part(zip_bytes: &[u8]) -> Result<String, ParseError> {
    let rels = parse_rels(&utf8(part_bytes(zip_bytes, "_rels/.rels")?, "_rels/.rels")?)?;
    find_target(&rels, RT_OFFICE_DOCUMENT, "")?
        .ok_or_else(|| ParseError("no officeDocument relationship in _rels/.rels".into()))
}
