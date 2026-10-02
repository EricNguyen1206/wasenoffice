//! Slice tests (T-A20, DD §7): fixture chuẩn (tiếng Việt gộp) + edge XML +
//! construct semantics. 3 tests, không framework.

use std::io::Write as _;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/nanooffice-fixture.docx"
);

/// Fixture chuẩn (sinh bằng `harness/fixtures/make_docx.py` — spec-derived,
/// cùng file render GREEN trong engine): 5 paragraph, style, bold, tiếng Việt
/// có dấu, tab literal, smart quotes — text phải exact, không panic.
#[test]
fn fixture_parses_to_exact_model() {
    let doc = nanooffice_ooxml::parse_docx_file(FIXTURE).unwrap();
    assert_eq!(doc.paragraphs.len(), 5);

    let p = &doc.paragraphs[0];
    assert_eq!(p.style.as_deref(), Some("Heading1"));
    assert_eq!(p.runs.len(), 1);
    assert_eq!(p.runs[0].text, "NanoOffice PH-0 Fixture");
    assert!(!p.runs[0].bold);

    let p = &doc.paragraphs[2];
    assert_eq!(p.style, None);
    assert_eq!(p.runs.len(), 1);
    assert!(p.runs[0].bold, "whole-sentence bold run");
    assert_eq!(
        p.runs[0].text,
        "Bold run check: this whole sentence should be bold."
    );

    let p = &doc.paragraphs[3];
    assert_eq!(
        p.runs[0].text,
        "Đoạn tiếng Việt có dấu: ẽnghệ ộ lại là là — kiểm tra font fallback."
    );

    let p = &doc.paragraphs[4];
    assert_eq!(
        p.runs[0].text,
        "0123456789 — tab\there — quotes “smart” and 'plain'."
    );
}

/// Trust boundary: malformed input → `Err`, never panic.
#[test]
fn malformed_input_is_an_error_not_a_panic() {
    // (a) garbage XML
    let err = parse_document("this is not xml at all <<<").unwrap_err();
    assert!(err.0.contains("w:document"), "unexpected: {err:?}");
    // (b) valid zip, wrong part name
    let zip = docx_zip("word/wrong.xml", "<w:document/>");
    let err = nanooffice_ooxml::parse_docx(&zip).unwrap_err();
    assert!(
        err.0.contains("part word/document.xml"),
        "unexpected: {err:?}"
    );
    // (c) truncated zip
    let zip = docx_zip("word/document.xml", "<w:document/>");
    let err = nanooffice_ooxml::parse_docx(&zip[..zip.len() / 2]).unwrap_err();
    assert!(err.0.contains("zip"), "unexpected: {err:?}");
}

/// Construct semantics: pStyle qua pPr, OnOff `w:val` (false/0 tắt), `w:i`,
/// empty `<w:p/>`, entities trong `w:t`.
#[test]
fn construct_semantics() {
    let xml = concat!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
        r#"<w:body>"#,
        r#"<w:p><w:pPr><w:pStyle w:val="Heading2"/></w:pPr><w:r><w:t xml:space="preserve">styled</w:t></w:r></w:p>"#,
        r#"<w:p>"#,
        r#"<w:r><w:rPr><w:b w:val="false"/></w:rPr><w:t>not bold</w:t></w:r>"#,
        r#"<w:r><w:rPr><w:i/></w:rPr><w:t>italic</w:t></w:r>"#,
        r#"<w:r><w:rPr><w:b/><w:i w:val="0"/></w:rPr><w:t>bold only</w:t></w:r>"#,
        r#"</w:p>"#,
        r#"<w:p/>"#,
        r#"<w:p><w:r><w:t>a &amp; b &lt;c&gt;</w:t></w:r></w:p>"#,
        r#"</w:body></w:document>"#,
    );
    let doc = nanooffice_ooxml::parse_docx(&docx_zip("word/document.xml", xml)).unwrap();
    assert_eq!(doc.paragraphs.len(), 4);

    let p = &doc.paragraphs[0];
    assert_eq!(p.style.as_deref(), Some("Heading2"));
    assert_eq!(p.runs[0].text, "styled");

    let runs = &doc.paragraphs[1].runs;
    assert_eq!(runs.len(), 3);
    assert_eq!(runs[0].text, "not bold");
    assert!(!runs[0].bold && !runs[0].italic);
    assert_eq!(runs[1].text, "italic");
    assert!(runs[1].italic && !runs[1].bold);
    assert_eq!(runs[2].text, "bold only");
    assert!(runs[2].bold && !runs[2].italic, "w:val=0 must turn w:i off");

    assert_eq!(doc.paragraphs[2].runs.len(), 0, "<w:p/> = empty paragraph");
    assert_eq!(doc.paragraphs[3].runs[0].text, "a & b <c>");
}

// helpers — in-memory zip (STORED) chỉ cho test
fn parse_document(xml: &str) -> Result<nanooffice_ooxml::Document, nanooffice_ooxml::ParseError> {
    let zip = docx_zip("word/document.xml", xml);
    nanooffice_ooxml::parse_docx(&zip)
}

fn docx_zip(name: &str, content: &str) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    w.start_file(name, zip::write::SimpleFileOptions::default())
        .unwrap();
    w.write_all(content.as_bytes()).unwrap();
    w.finish().unwrap().into_inner()
}
