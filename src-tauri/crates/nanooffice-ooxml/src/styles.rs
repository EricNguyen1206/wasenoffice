//! `word/styles.xml` + `word/numbering.xml` → resolved dictionaries
//! (ECMA-376 Part 1: styles §17.7, numbering §17.9).
//!
//! basedOn chains (§17.7.4.3) are resolved eagerly: the child style
//! inherits the parent's properties and a declared child value wins.
//! A cycle or a dangling basedOn reference is an error, not a guess.

use std::collections::{BTreeMap, BTreeSet};

use quick_xml::events::{BytesStart, Event};

use crate::document::onoff;
use crate::opc::attr;
use crate::{xml_err, NumLvl, ParseError, StyleDef, StyleKind};

/// Raw style as written in styles.xml, before chain resolution.
#[derive(Debug, Default)]
struct RawStyle {
    kind: StyleKind,
    name: Option<String>,
    based_on: Option<String>,
    align: Option<String>,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<String>,
    size: Option<u32>,
}

/// Parse styles.xml: `w:style` (§17.7.4.17) entries keyed by styleId. Only
/// paragraph + character styles are modelled (S1 surface); other style types
/// (table, numbering) are skipped as out-of-scope rather than half-modelled.
pub fn parse_styles_xml(xml: &str) -> Result<BTreeMap<String, StyleDef>, ParseError> {
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut raw: BTreeMap<String, RawStyle> = BTreeMap::new();
    let mut saw_root = false;
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf).map_err(xml_err)? {
            Event::Start(e) => {
                if e.name().as_ref() == b"w:styles" {
                    saw_root = true;
                } else if e.name().as_ref() == b"w:style" && saw_root {
                    parse_style(&mut reader, e, &mut raw)?;
                } else {
                    skip(&mut reader)?;
                }
            }
            // `<w:styles/>` — a styles part with no styles is well-formed
            Event::Empty(e) => {
                if e.name().as_ref() == b"w:styles" {
                    saw_root = true;
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !saw_root {
        return Err(ParseError("no w:styles root — not a styles part".into()));
    }

    let mut resolved = BTreeMap::new();
    for id in raw.keys().cloned().collect::<Vec<_>>() {
        let mut path = BTreeSet::new();
        resolved.insert(id.clone(), resolve_style(&id, &raw, &mut path)?);
    }
    Ok(resolved)
}

/// One `w:style` (§17.7.4.17): name (§17.7.4.9), basedOn (§17.7.4.3), and
/// the pPr/rPr subset S1 models. Non-paragraph/character types are skipped.
fn parse_style(
    reader: &mut quick_xml::Reader<&[u8]>,
    open: BytesStart<'_>,
    raw: &mut BTreeMap<String, RawStyle>,
) -> Result<(), ParseError> {
    let style_type = attr(&open, b"w:type")?;
    let kind = match style_type.as_deref() {
        Some("paragraph") => StyleKind::Paragraph,
        Some("character") => StyleKind::Character,
        // table/numbering styles stay out of the model (S1 surface)
        _ => return skip(reader),
    };
    let id =
        attr(&open, b"w:styleId")?.ok_or_else(|| ParseError("w:style missing w:styleId".into()))?;
    let mut style = RawStyle {
        kind,
        ..Default::default()
    };
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf).map_err(xml_err)? {
            Event::Start(e) => match e.name().as_ref() {
                b"w:pPr" => parse_style_ppr(reader, &mut style)?,
                b"w:rPr" => parse_style_rpr(reader, &mut style)?,
                _ => skip(reader)?,
            },
            Event::Empty(e) => match e.name().as_ref() {
                b"w:name" => style.name = attr(&e, b"w:val")?,
                b"w:basedOn" => style.based_on = attr(&e, b"w:val")?,
                b"w:jc" => style.align = attr(&e, b"w:val")?,
                b"w:b" => style.bold = Some(onoff(&e)?),
                b"w:i" => style.italic = Some(onoff(&e)?),
                b"w:u" => {
                    style.underline =
                        Some(attr(&e, b"w:val")?.unwrap_or_else(|| "single".to_string()))
                }
                b"w:sz" => style.size = Some(attr_u32(&e, b"w:val")?),
                _ => {}
            },
            Event::End(e) if e.name().as_ref() == b"w:style" => break,
            Event::Eof => return Err(ParseError("unexpected end of XML inside w:style".into())),
            _ => {}
        }
    }
    raw.insert(id, style);
    Ok(())
}

/// Style-level `w:pPr` (§17.7.8.2): S1 reads only `w:jc` (§17.3.1.13).
fn parse_style_ppr(
    reader: &mut quick_xml::Reader<&[u8]>,
    style: &mut RawStyle,
) -> Result<(), ParseError> {
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf).map_err(xml_err)? {
            Event::Start(e) => match e.name().as_ref() {
                b"w:jc" => style.align = attr(&e, b"w:val")?,
                _ => skip(reader)?,
            },
            Event::Empty(e) if e.name().as_ref() == b"w:jc" => {
                style.align = attr(&e, b"w:val")?;
            }
            Event::End(e) if e.name().as_ref() == b"w:pPr" => return Ok(()),
            Event::Eof => return Err(ParseError("unexpected end of XML inside style pPr".into())),
            _ => {}
        }
    }
}

/// Style-level `w:rPr` (§17.7.9.1): S1 reads b/i/u/sz.
fn parse_style_rpr(
    reader: &mut quick_xml::Reader<&[u8]>,
    style: &mut RawStyle,
) -> Result<(), ParseError> {
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf).map_err(xml_err)? {
            Event::Start(e) => match e.name().as_ref() {
                b"w:b" => style.bold = Some(onoff(&e)?),
                b"w:i" => style.italic = Some(onoff(&e)?),
                b"w:u" => {
                    style.underline =
                        Some(attr(&e, b"w:val")?.unwrap_or_else(|| "single".to_string()))
                }
                b"w:sz" => style.size = Some(attr_u32(&e, b"w:val")?),
                _ => skip(reader)?,
            },
            Event::Empty(e) => match e.name().as_ref() {
                b"w:b" => style.bold = Some(onoff(&e)?),
                b"w:i" => style.italic = Some(onoff(&e)?),
                b"w:u" => {
                    style.underline =
                        Some(attr(&e, b"w:val")?.unwrap_or_else(|| "single".to_string()))
                }
                b"w:sz" => style.size = Some(attr_u32(&e, b"w:val")?),
                _ => {}
            },
            Event::End(e) if e.name().as_ref() == b"w:rPr" => return Ok(()),
            Event::Eof => return Err(ParseError("unexpected end of XML inside style rPr".into())),
            _ => {}
        }
    }
}

/// Resolve one style through its basedOn chain: child values win over the
/// parent's (§17.7.4.3). Cycles and dangling references are errors.
fn resolve_style(
    id: &str,
    raw: &BTreeMap<String, RawStyle>,
    path: &mut BTreeSet<String>,
) -> Result<StyleDef, ParseError> {
    if !path.insert(id.to_string()) {
        return Err(ParseError(format!("style basedOn cycle through {id}")));
    }
    let style = raw
        .get(id)
        .ok_or_else(|| ParseError(format!("style {id} referenced via basedOn but not defined")))?;
    let parent = match &style.based_on {
        Some(pid) => Some(resolve_style(pid, raw, path)?),
        None => None,
    };
    path.remove(id);
    Ok(StyleDef {
        kind: style.kind, // the type is identity, not an inherited property
        name: style
            .name
            .clone()
            .or(parent.as_ref().and_then(|p| p.name.clone())),
        align: style
            .align
            .clone()
            .or(parent.as_ref().and_then(|p| p.align.clone())),
        bold: style.bold.or(parent.as_ref().and_then(|p| p.bold)),
        italic: style.italic.or(parent.as_ref().and_then(|p| p.italic)),
        underline: style
            .underline
            .clone()
            .or(parent.as_ref().and_then(|p| p.underline.clone())),
        size: style.size.or(parent.as_ref().and_then(|p| p.size)),
    })
}

/// Parse numbering.xml (§17.9): `w:abstractNum` (§17.9.1) levels (§17.9.7)
/// + `w:num` (§17.9.16) instances → resolved numId → ilvl → format.
///
/// A num pointing at a missing abstractNum is skipped (the paragraph keeps
/// its raw numPr; a missing dictionary entry is data, not a parse error).
pub fn parse_numbering_xml(xml: &str) -> Result<BTreeMap<u32, BTreeMap<u32, NumLvl>>, ParseError> {
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut abstracts: BTreeMap<u32, BTreeMap<u32, NumLvl>> = BTreeMap::new();
    let mut resolved: BTreeMap<u32, BTreeMap<u32, NumLvl>> = BTreeMap::new();
    let mut saw_root = false;
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf).map_err(xml_err)? {
            Event::Start(e) => {
                if e.name().as_ref() == b"w:numbering" {
                    saw_root = true;
                } else if e.name().as_ref() == b"w:abstractNum" && saw_root {
                    parse_abstract_num(&mut reader, &e, &mut abstracts)?;
                } else if e.name().as_ref() == b"w:num" && saw_root {
                    parse_num(&mut reader, &e, &abstracts, &mut resolved)?;
                } else {
                    skip(&mut reader)?;
                }
            }
            Event::Empty(e) => {
                if e.name().as_ref() == b"w:numbering" {
                    saw_root = true;
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !saw_root {
        return Err(ParseError(
            "no w:numbering root — not a numbering part".into(),
        ));
    }
    Ok(resolved)
}

/// `w:abstractNum` (§17.9.1): its `w:lvl` (§17.9.7) children carry numFmt
/// (§17.9.18) + lvlText (§17.9.12). Levels missing either are skipped.
fn parse_abstract_num(
    reader: &mut quick_xml::Reader<&[u8]>,
    open: &BytesStart<'_>,
    abstracts: &mut BTreeMap<u32, BTreeMap<u32, NumLvl>>,
) -> Result<(), ParseError> {
    let id = attr_u32(open, b"w:abstractNumId")?;
    let mut lvls: BTreeMap<u32, NumLvl> = BTreeMap::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf).map_err(xml_err)? {
            Event::Start(e) | Event::Empty(e) => {
                if e.name().as_ref() == b"w:lvl" {
                    let ilvl = attr_u32(&e, b"w:ilvl")?;
                    if let Some(lvl) = parse_lvl(reader)? {
                        lvls.insert(ilvl, lvl);
                    }
                }
            }
            Event::End(e) if e.name().as_ref() == b"w:abstractNum" => break,
            Event::Eof => {
                return Err(ParseError(
                    "unexpected end of XML inside w:abstractNum".into(),
                ))
            }
            _ => {}
        }
    }
    abstracts.insert(id, lvls);
    Ok(())
}

/// One `w:lvl` (§17.9.7): numFmt (§17.9.18) + lvlText (§17.9.12); a level
/// missing either element is reported as None and skipped.
fn parse_lvl(reader: &mut quick_xml::Reader<&[u8]>) -> Result<Option<NumLvl>, ParseError> {
    let mut buf = Vec::new();
    let mut fmt = None;
    let mut text = None;
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf).map_err(xml_err)? {
            Event::Start(e) | Event::Empty(e) => match e.name().as_ref() {
                b"w:numFmt" => fmt = attr(&e, b"w:val")?,
                b"w:lvlText" => text = attr(&e, b"w:val")?,
                _ => {}
            },
            Event::End(e) if e.name().as_ref() == b"w:lvl" => {
                return Ok(fmt
                    .zip(text)
                    .map(|(num_fmt, lvl_text)| NumLvl { num_fmt, lvl_text }))
            }
            Event::Eof => return Err(ParseError("unexpected end of XML inside w:lvl".into())),
            _ => {}
        }
    }
}

/// `w:num` (§17.9.16): numId → abstractNumId (§17.9.2) → resolved levels.
/// A num whose abstractNum is missing is skipped (see module doc).
fn parse_num(
    reader: &mut quick_xml::Reader<&[u8]>,
    open: &BytesStart<'_>,
    abstracts: &BTreeMap<u32, BTreeMap<u32, NumLvl>>,
    resolved: &mut BTreeMap<u32, BTreeMap<u32, NumLvl>>,
) -> Result<(), ParseError> {
    let num_id = attr_u32(open, b"w:numId")?;
    let mut abstract_id = None;
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf).map_err(xml_err)? {
            Event::Start(e) | Event::Empty(e) => {
                if e.name().as_ref() == b"w:abstractNumId" {
                    abstract_id = attr(&e, b"w:val")?.and_then(|v| v.parse().ok());
                }
            }
            Event::End(e) if e.name().as_ref() == b"w:num" => {
                if let Some(id) = abstract_id {
                    if let Some(lvls) = abstracts.get(&id) {
                        resolved.insert(num_id, lvls.clone());
                    }
                }
                return Ok(());
            }
            Event::Eof => return Err(ParseError("unexpected end of XML inside w:num".into())),
            _ => {}
        }
    }
}

fn attr_u32(e: &BytesStart<'_>, key: &[u8]) -> Result<u32, ParseError> {
    let v = attr(e, key)?.ok_or_else(|| {
        ParseError(format!(
            "missing {} on {}",
            String::from_utf8_lossy(key),
            String::from_utf8_lossy(e.name().as_ref())
        ))
    })?;
    v.parse().map_err(|_| {
        ParseError(format!(
            "{0}={v:?} is not a u32",
            String::from_utf8_lossy(key)
        ))
    })
}

/// Skip the subtree rooted at the `Start` event the caller just consumed.
fn skip(reader: &mut quick_xml::Reader<&[u8]>) -> Result<(), ParseError> {
    let mut buf = Vec::new();
    let mut depth = 1usize;
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf).map_err(xml_err)? {
            Event::Start(_) => depth += 1,
            Event::End(_) => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Ok(());
                }
            }
            Event::Eof => return Err(ParseError("unexpected end of XML while skipping".into())),
            _ => {}
        }
    }
}
