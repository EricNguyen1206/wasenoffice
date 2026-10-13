//! `word/styles.xml` + `word/numbering.xml` → resolved dictionaries
//! (ECMA-376 Part 1: styles §17.7, numbering §17.9).
//!
//! basedOn chains (§17.7.4.3) are resolved eagerly: the child style
//! inherits the parent's properties and a declared child value wins.
//! A cycle or a dangling basedOn reference is an error, not a guess.

use std::collections::{BTreeMap, BTreeSet};

use quick_xml::events::{BytesStart, Event};

use crate::document::{attr_i32, onoff, parse_ind, parse_rfonts};
use crate::opc::attr;
use crate::{xml_err, DocDefaults, Ind, NumLvl, ParseError, RFonts, StyleDef, StyleKind};

/// Run properties shared by a style's `w:rPr` (§17.7.9.1) and
/// `w:docDefaults/w:rPrDefault/w:rPr` (§17.7.5.3) — same subset, one parser.
#[derive(Debug, Default)]
struct RawProps {
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<String>,
    size: Option<u32>,
    rfonts: Option<RFonts>,
    color: Option<String>,
    spacing: Option<i32>,
}

/// Paragraph properties shared by a style's `w:pPr` (§17.7.8.2) and
/// `w:docDefaults/w:pPrDefault/w:pPr` (§17.7.5.2).
#[derive(Debug, Default)]
struct RawPPr {
    align: Option<String>,
    ind: Option<Ind>,
}

/// Raw style as written in styles.xml, before chain resolution.
#[derive(Debug, Default)]
struct RawStyle {
    kind: StyleKind,
    name: Option<String>,
    based_on: Option<String>,
    /// `w:default` (§17.7.2.4) — default style of its type.
    default: bool,
    props: RawProps,
    ppr: RawPPr,
}

/// Parse styles.xml: `w:style` (§17.7.4.17) entries keyed by styleId plus the
/// `w:docDefaults` (§17.7.5.1) block. Only paragraph + character styles are
/// modelled (S1 surface); other style types (table, numbering) are skipped as
/// out-of-scope rather than half-modelled.
pub fn parse_styles_xml(
    xml: &str,
) -> Result<(BTreeMap<String, StyleDef>, Option<DocDefaults>), ParseError> {
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut raw: BTreeMap<String, RawStyle> = BTreeMap::new();
    let mut doc_defaults: Option<DocDefaults> = None;
    let mut saw_root = false;
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf).map_err(xml_err)? {
            Event::Start(e) => {
                if e.name().as_ref() == b"w:styles" {
                    saw_root = true;
                } else if e.name().as_ref() == b"w:style" && saw_root {
                    parse_style(&mut reader, e, &mut raw)?;
                } else if e.name().as_ref() == b"w:docDefaults" && saw_root {
                    doc_defaults = Some(parse_doc_defaults(&mut reader)?);
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
    Ok((resolved, doc_defaults))
}

/// `w:docDefaults` (§17.7.5.1): optional `w:rPrDefault/w:rPr` (§17.7.5.3) and
/// `w:pPrDefault/w:pPr` (§17.7.5.2) — the same property subsets a style
/// carries, parsed by the same functions.
fn parse_doc_defaults(reader: &mut quick_xml::Reader<&[u8]>) -> Result<DocDefaults, ParseError> {
    let mut props = RawProps::default();
    let mut ppr = RawPPr::default();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf).map_err(xml_err)? {
            Event::Start(e) => match e.name().as_ref() {
                b"w:rPrDefault" => parse_rpr_default(reader, &mut props)?,
                b"w:pPrDefault" => parse_ppr_default(reader, &mut ppr)?,
                _ => skip(reader)?,
            },
            Event::End(e) if e.name().as_ref() == b"w:docDefaults" => {
                return Ok(DocDefaults {
                    bold: props.bold,
                    italic: props.italic,
                    underline: props.underline,
                    size: props.size,
                    rfonts: props.rfonts,
                    color: props.color,
                    spacing: props.spacing,
                    align: ppr.align,
                    ind: ppr.ind,
                })
            }
            Event::Eof => {
                return Err(ParseError(
                    "unexpected end of XML inside w:docDefaults".into(),
                ))
            }
            _ => {}
        }
    }
}

/// `w:rPrDefault` (§17.7.5.3) → its single `w:rPr` child.
fn parse_rpr_default(
    reader: &mut quick_xml::Reader<&[u8]>,
    props: &mut RawProps,
) -> Result<(), ParseError> {
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf).map_err(xml_err)? {
            Event::Start(e) => {
                if e.name().as_ref() == b"w:rPr" {
                    parse_props_rpr(reader, props)?;
                } else {
                    skip(reader)?;
                }
            }
            Event::End(e) if e.name().as_ref() == b"w:rPrDefault" => return Ok(()),
            Event::Eof => {
                return Err(ParseError(
                    "unexpected end of XML inside w:rPrDefault".into(),
                ))
            }
            _ => {}
        }
    }
}

/// `w:pPrDefault` (§17.7.5.2) → its single `w:pPr` child.
fn parse_ppr_default(
    reader: &mut quick_xml::Reader<&[u8]>,
    ppr: &mut RawPPr,
) -> Result<(), ParseError> {
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf).map_err(xml_err)? {
            Event::Start(e) => {
                if e.name().as_ref() == b"w:pPr" {
                    parse_props_ppr(reader, ppr)?;
                } else {
                    skip(reader)?;
                }
            }
            Event::End(e) if e.name().as_ref() == b"w:pPrDefault" => return Ok(()),
            Event::Eof => {
                return Err(ParseError(
                    "unexpected end of XML inside w:pPrDefault".into(),
                ))
            }
            _ => {}
        }
    }
}

/// One `w:style` (§17.7.4.17): name (§17.7.4.9), basedOn (§17.7.4.3),
/// `w:default` (§17.7.2.4), and the pPr/rPr subset the model carries.
/// Non-paragraph/character types are skipped.
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
    // `w:default` (§17.7.2.4) is an ATTRIBUTE here, not an OnOff element: an
    // absent attribute means false (unlike `<w:b/>`, where an absent w:val
    // means on).
    let default = match attr(&open, b"w:default")?.as_deref() {
        None | Some("false") | Some("0") | Some("off") => false,
        Some("true") | Some("1") | Some("on") => true,
        Some(other) => {
            return Err(ParseError(format!(
                "w:default={other:?} is not an OnOff value"
            )))
        }
    };
    let mut style = RawStyle {
        kind,
        default,
        ..Default::default()
    };
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf).map_err(xml_err)? {
            Event::Start(e) => match e.name().as_ref() {
                b"w:pPr" => parse_props_ppr(reader, &mut style.ppr)?,
                b"w:rPr" => parse_props_rpr(reader, &mut style.props)?,
                _ => skip(reader)?,
            },
            Event::Empty(e) => match e.name().as_ref() {
                b"w:name" => style.name = attr(&e, b"w:val")?,
                b"w:basedOn" => style.based_on = attr(&e, b"w:val")?,
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

/// Style-level and docDefaults-level `w:pPr`: jc (§17.3.1.13) + ind
/// (§17.3.1.12).
fn parse_props_ppr(
    reader: &mut quick_xml::Reader<&[u8]>,
    ppr: &mut RawPPr,
) -> Result<(), ParseError> {
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf).map_err(xml_err)? {
            Event::Start(e) => match e.name().as_ref() {
                b"w:jc" => ppr.align = attr(&e, b"w:val")?,
                b"w:ind" => ppr.ind = Some(parse_ind(&e)?),
                _ => skip(reader)?,
            },
            Event::Empty(e) => match e.name().as_ref() {
                b"w:jc" => ppr.align = attr(&e, b"w:val")?,
                b"w:ind" => ppr.ind = Some(parse_ind(&e)?),
                _ => {}
            },
            Event::End(e) if e.name().as_ref() == b"w:pPr" => return Ok(()),
            Event::Eof => return Err(ParseError("unexpected end of XML inside style pPr".into())),
            _ => {}
        }
    }
}

/// Style-level and docDefaults-level `w:rPr`: rStyle-less subset — b/i/u/sz,
/// rFonts (§17.3.2.26), color (§17.3.2.6), spacing (§17.3.2.33).
fn parse_props_rpr(
    reader: &mut quick_xml::Reader<&[u8]>,
    props: &mut RawProps,
) -> Result<(), ParseError> {
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf).map_err(xml_err)? {
            Event::Start(e) | Event::Empty(e) => match e.name().as_ref() {
                b"w:b" => props.bold = Some(onoff(&e)?),
                b"w:i" => props.italic = Some(onoff(&e)?),
                b"w:u" => {
                    props.underline =
                        Some(attr(&e, b"w:val")?.unwrap_or_else(|| "single".to_string()))
                }
                b"w:sz" => props.size = Some(attr_u32(&e, b"w:val")?),
                b"w:rFonts" => props.rfonts = Some(parse_rfonts(&e)?),
                b"w:color" => props.color = attr(&e, b"w:val")?,
                b"w:spacing" => props.spacing = Some(attr_i32(&e, b"w:val")?),
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
        default: style.default,
        name: style
            .name
            .clone()
            .or(parent.as_ref().and_then(|p| p.name.clone())),
        align: style
            .ppr
            .align
            .clone()
            .or(parent.as_ref().and_then(|p| p.align.clone())),
        ind: style
            .ppr
            .ind
            .clone()
            .or(parent.as_ref().and_then(|p| p.ind.clone())),
        bold: style.props.bold.or(parent.as_ref().and_then(|p| p.bold)),
        italic: style
            .props
            .italic
            .or(parent.as_ref().and_then(|p| p.italic)),
        underline: style
            .props
            .underline
            .clone()
            .or(parent.as_ref().and_then(|p| p.underline.clone())),
        size: style.props.size.or(parent.as_ref().and_then(|p| p.size)),
        rfonts: style
            .props
            .rfonts
            .clone()
            .or(parent.as_ref().and_then(|p| p.rfonts.clone())),
        color: style
            .props
            .color
            .clone()
            .or(parent.as_ref().and_then(|p| p.color.clone())),
        spacing: style
            .props
            .spacing
            .or(parent.as_ref().and_then(|p| p.spacing)),
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
