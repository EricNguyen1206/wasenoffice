//! `word/document.xml` → model (ECMA-376 Part 1, WordprocessingML §17).
//!
//! Elements/attributes are matched by their prefixed names and the root must
//! bind the `w:` prefix to the WordprocessingML namespace — a document that
//! binds a different prefix is refused with an error rather than parsed
//! silently wrong (DD §6 "refuse-unknown instead of parse-wrong").
//! OnOff semantics (`w:b`/`w:i`, §17.3.2.1): val absent or true/1/on = on;
//! false/0/off = off.

use quick_xml::events::{BytesStart, Event};

use crate::{Document, Paragraph, ParseError, Run};

const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

pub fn parse_document_xml(xml: &str) -> Result<Document, ParseError> {
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut buf = Vec::new();

    let mut saw_document = false;
    let mut paragraphs: Vec<Paragraph> = Vec::new();
    // per-paragraph state
    let mut style: Option<String> = None;
    let mut runs: Vec<Run> = Vec::new();
    // per-run state
    let mut in_run = false;
    let mut in_rpr = false; // the RUN's rPr (a pPr's rPr styles the mark, not runs)
    let mut bold = false;
    let mut italic = false;
    // text collection
    let mut in_ppr = false;
    let mut in_t = false;
    let mut text = String::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"w:document" => {
                    saw_document = true;
                    if attr(&e, b"xmlns:w")?.as_deref() != Some(W_NS) {
                        return Err(ParseError(
                            "w: prefix is not bound to the WordprocessingML namespace — refusing to parse with wrong namespaces".into(),
                        ));
                    }
                }
                b"w:p" => {
                    style = None;
                    runs = Vec::new();
                }
                b"w:pPr" => in_ppr = true,
                b"w:pStyle" => {
                    if in_ppr {
                        style = attr(&e, b"w:val")?;
                    }
                }
                b"w:r" => {
                    in_run = true;
                    bold = false;
                    italic = false;
                }
                b"w:rPr" => in_rpr = in_run,
                b"w:b" => {
                    if in_rpr {
                        bold = onoff(&e)?;
                    }
                }
                b"w:i" => {
                    if in_rpr {
                        italic = onoff(&e)?;
                    }
                }
                b"w:t" => {
                    in_t = true;
                    text.clear();
                }
                _ => {}
            },
            Ok(Event::Empty(e)) => match e.name().as_ref() {
                b"w:p" => paragraphs.push(Paragraph {
                    style: None,
                    runs: Vec::new(),
                }),
                b"w:pStyle" => {
                    if in_ppr {
                        style = attr(&e, b"w:val")?;
                    }
                }
                b"w:b" => {
                    if in_rpr {
                        bold = onoff(&e)?;
                    }
                }
                b"w:i" => {
                    if in_rpr {
                        italic = onoff(&e)?;
                    }
                }
                _ => {}
            },
            Ok(Event::Text(t)) => {
                if in_t {
                    text.push_str(
                        &t.unescape()
                            .map_err(|e| ParseError(format!("xml text: {e}")))?,
                    );
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"w:t" => {
                    in_t = false;
                    if in_run {
                        runs.push(Run {
                            text: text.clone(),
                            bold,
                            italic,
                        });
                    }
                }
                b"w:r" | b"w:rPr" => {
                    in_rpr = false;
                    if e.name().as_ref() == b"w:r" {
                        in_run = false;
                    }
                }
                b"w:pPr" => in_ppr = false,
                b"w:p" => paragraphs.push(Paragraph {
                    style: style.take(),
                    runs: std::mem::take(&mut runs),
                }),
                _ => {}
            },
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(ParseError(format!("xml: {e}"))),
        }
    }

    if !saw_document {
        return Err(ParseError(
            "no w:document root — not a wordprocessingml document".into(),
        ));
    }
    Ok(Document { paragraphs })
}

/// One attribute by its prefixed key, unescaped.
fn attr(e: &BytesStart<'_>, key: &[u8]) -> Result<Option<String>, ParseError> {
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

/// OnOff value of a formatting element (`<w:b/>`, `<w:b w:val="…"/>`).
fn onoff(e: &BytesStart<'_>) -> Result<bool, ParseError> {
    Ok(match attr(e, b"w:val")?.as_deref() {
        None | Some("true") | Some("1") | Some("on") => true,
        Some("false") | Some("0") | Some("off") => false,
        Some(other) => return Err(ParseError(format!("w:val={other:?} is not an OnOff value"))),
    })
}
