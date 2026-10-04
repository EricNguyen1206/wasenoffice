//! Write-side slice tests (T-A22, DD §7): round-trip model bất biến trên
//! corpus đầy đủ (điều kiện 1 DD §4) + edge ký tự XML đặc biệt/tiếng Việt +
//! fuzz mutation trên model. 3 test, không framework.

use std::collections::BTreeMap;
use std::io::Read as _;

use nanooffice_ooxml::{
    write_docx, Block, Document, HeaderFooter, ImageRef, NumLvl, NumRef, Paragraph, Run, StyleDef,
    StyleKind, Table, TableCell,
};

const FULL_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/nanooffice-full-fixture.docx"
);

const MEDIA_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/nanooffice-media-fixture.docx"
);

const FIDELITY_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/nanooffice-fidelity-fixture.docx"
);

/// Điều kiện XONG (1) của DD §4: `parse → write → parse` trên corpus 10
/// parts — model bất biến 100% (table/list/image/hdr-ftr refs + tiếng Việt).
/// Package shape cũng được kiểm: đúng phần OPC chuẩn, media part là PNG.
#[test]
fn round_trip_full_corpus_model_invariant() {
    let doc1 = nanooffice_ooxml::parse_docx_file(FULL_FIXTURE).unwrap();
    let bytes = write_docx(&doc1, None).unwrap();
    let doc2 = nanooffice_ooxml::parse_docx(&bytes).unwrap();
    assert_eq!(doc1, doc2);

    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes[..])).unwrap();
    for part in [
        "[Content_Types].xml",
        "_rels/.rels",
        "word/_rels/document.xml.rels",
        "word/document.xml",
        "word/styles.xml",
        "word/numbering.xml",
        "word/header1.xml",
        "word/footer1.xml",
        "word/media/image1.png",
    ] {
        assert!(zip.by_name(part).is_ok(), "missing part {part}");
    }
    let png = zip.by_name("word/media/image1.png").unwrap();
    assert!(
        (50..200).contains(&png.size()),
        "placeholder png size {}",
        png.size()
    );
}

/// T-A31 (DD L01 §5.6): fidelity corpus round-trip giữ model + hyperlink
/// element + TargetMode External, và settings/theme/docProps đi qua
/// BYTE-EXACT từ package nguồn.
#[test]
fn fidelity_roundtrip_keeps_links_sections_and_passthrough() {
    let src = std::fs::read(FIDELITY_FIXTURE).unwrap();
    let doc1 = nanooffice_ooxml::parse_docx(&src).unwrap();
    let out = write_docx(&doc1, Some(&src)).unwrap();
    assert_eq!(doc1, nanooffice_ooxml::parse_docx(&out).unwrap());

    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&out[..])).unwrap();
    let mut rels = String::new();
    zip.by_name("word/_rels/document.xml.rels")
        .unwrap()
        .read_to_string(&mut rels)
        .unwrap();
    assert!(
        rels.contains("TargetMode=\"External\"") && rels.contains("nanooffice-fidelity"),
        "hyperlink rel must keep its external target: {rels}"
    );
    assert!(rels.contains("relationships/settings") && rels.contains("relationships/theme"));
    let mut pkg = String::new();
    zip.by_name("_rels/.rels")
        .unwrap()
        .read_to_string(&mut pkg)
        .unwrap();
    assert!(pkg.contains("metadata/core-properties") && pkg.contains("extended-properties"));
    for part in [
        "word/settings.xml",
        "word/theme/theme1.xml",
        "docProps/core.xml",
        "docProps/app.xml",
    ] {
        let mut got = Vec::new();
        zip.by_name(part).unwrap().read_to_end(&mut got).unwrap();
        assert_eq!(
            got,
            nanooffice_ooxml::try_part_bytes(&src, part).unwrap(),
            "{part} must survive byte-exact"
        );
    }
}

/// T-A31: một package NGUỒN không mang settings/theme/docProps → writer
/// không bịa part nào (passthrough là copy-when-present).
#[test]
fn passthrough_is_silent_when_the_source_lacks_the_parts() {
    let doc = nanooffice_ooxml::parse_docx_file(FULL_FIXTURE).unwrap();
    let src = std::fs::read(FULL_FIXTURE).unwrap();
    let out = write_docx(&doc, Some(&src)).unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&out[..])).unwrap();
    assert!(zip.by_name("word/settings.xml").is_err());
    assert!(zip.by_name("docProps/core.xml").is_err());
}

/// T-A26: media part bytes của package nguồn phải sang qua canonical NGUYÊN
/// VẸN — so BYTES từng part (body lẫn header), không phải kích thước; model
/// bất biến qua round-trip có nguồn. Khi không có nguồn (`None`) writer rơi
/// về placeholder 1×1 (path fallback cho model synthetic) — bytes khác gốc.
#[test]
fn media_bytes_copied_byte_exact_from_source_package() {
    let src = std::fs::read(MEDIA_FIXTURE).unwrap();
    let doc = nanooffice_ooxml::parse_docx(&src).unwrap();
    let original = media_parts(&src);
    assert_eq!(original.len(), 2, "fixture: body + header media");

    let bytes = write_docx(&doc, Some(&src)).unwrap();
    assert_eq!(
        doc,
        nanooffice_ooxml::parse_docx(&bytes).unwrap(),
        "model invariant through the round-trip with a source"
    );
    assert_eq!(
        media_parts(&bytes),
        original,
        "media bytes changed through canonical"
    );

    assert_ne!(
        media_parts(&write_docx(&doc, None).unwrap()),
        original,
        "None must fall back to the placeholder"
    );
}

/// Every `word/media/*` part of a package, name → bytes.
fn media_parts(zip_bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).unwrap();
    let mut out = BTreeMap::new();
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).unwrap();
        if f.name().starts_with("word/media/") {
            let mut bytes = Vec::new();
            f.read_to_end(&mut bytes).unwrap();
            out.insert(f.name().to_string(), bytes);
        }
    }
    out
}

/// Trust boundary phía write: ký tự XML đặc biệt + tiếng Việt + whitespace
/// hai đầu phải qua escape/`xml:space` nguyên vẹn; image extension ngoài
/// bảng media type bị refuse thay vì đoán content type.
#[test]
fn write_edge_xml_special_and_vietnamese() {
    let mut styles = BTreeMap::new();
    styles.insert(
        "S&T".to_string(),
        StyleDef {
            kind: StyleKind::Character,
            name: Some("Name <&>".to_string()),
            bold: Some(false),
            italic: Some(true),
            ..Default::default()
        },
    );
    let mut numbering = BTreeMap::new();
    numbering.insert(
        9u32,
        BTreeMap::from([(
            3u32,
            NumLvl {
                num_fmt: "decimal".to_string(),
                lvl_text: "%1<&>".to_string(),
            },
        )]),
    );
    let doc = Document {
        body: vec![
            Block::Paragraph(Paragraph {
                style: Some("Head <ing> & \"style\"".to_string()),
                align: Some("both".to_string()),
                numbering: Some(NumRef { num_id: 7, ilvl: 3 }),
                ind: None,
                sect: None,
                runs: vec![
                    Run {
                        text: "a & b <c> \"d\" 'e' — Tiếng Việt có dấu: ẽnghệ ộ lại là là"
                            .to_string(),
                        bold: Some(true),
                        italic: Some(true),
                        underline: Some("single".to_string()),
                        size: Some(28),
                        ..Default::default()
                    },
                    Run {
                        text: "  leading & trailing  ".to_string(),
                        ..Default::default()
                    },
                ],
            }),
            Block::Table(Table {
                grid: Vec::new(),
                rows: vec![vec![
                    TableCell {
                        blocks: vec![Block::Paragraph(Paragraph {
                            runs: vec![Run {
                                text: "&<>\"' — ô bảng".to_string(),
                                ..Default::default()
                            }],
                            ..Default::default()
                        })],
                    },
                    TableCell::default(),
                ]],
            }),
        ],
        headers: vec![HeaderFooter {
            part: "word/header1.xml".to_string(),
            blocks: vec![Block::Paragraph(Paragraph {
                runs: vec![Run {
                    text: "header & <footer> 'quote'".to_string(),
                    ..Default::default()
                }],
                ..Default::default()
            })],
        }],
        footers: vec![],
        doc_defaults: None,
        page: None,
        page_mar: None,
        styles,
        numbering,
    };

    let bytes = write_docx(&doc, None).unwrap();
    let doc2 = nanooffice_ooxml::parse_docx(&bytes).unwrap();
    assert_eq!(doc, doc2);

    // escape thật sự nằm trong XML emitted (raw & không được lọt qua)
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes[..])).unwrap();
    let mut xml = String::new();
    zip.by_name("word/document.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    assert!(xml.contains("a &amp; b &lt;c&gt;"), "escaped: {xml}");
    assert!(!xml.contains("a & b"), "raw & must not appear");
    assert!(
        xml.contains("<w:t xml:space=\"preserve\">  leading &amp; trailing  </w:t>"),
        "leading/trailing whitespace needs xml:space preserve"
    );

    // unknown image extension → WriteError, không panic
    let bad = Document {
        body: vec![Block::Paragraph(Paragraph {
            runs: vec![Run {
                images: vec![ImageRef {
                    rel_id: "rIdX".to_string(),
                    part: "word/media/pic.webp".to_string(),
                    cx_emu: 1,
                    cy_emu: 1,
                }],
                ..Default::default()
            }],
            ..Default::default()
        })],
        ..empty_model()
    };
    let err = write_docx(&bad, None).unwrap_err();
    assert!(err.0.contains("webp"), "unexpected: {err:?}");
}

/// Fuzz phía write: mutation trên model (seed cố định, LCG tự viết) —
/// write không panic, reparse luôn thành công, model bất biến từng vòng.
#[test]
fn fuzz_write_reparse_never_panics_and_preserves_model() {
    let base = Document {
        body: vec![
            Block::Paragraph(Paragraph {
                style: Some("H1".to_string()),
                align: Some("center".to_string()),
                numbering: Some(NumRef { num_id: 1, ilvl: 0 }),
                ind: None,
                sect: None,
                runs: vec![Run {
                    text: "Tiếng Việt & <junk> “smart” — base".to_string(),
                    bold: Some(true),
                    ..Default::default()
                }],
            }),
            Block::Table(Table {
                grid: Vec::new(),
                rows: vec![vec![TableCell {
                    blocks: vec![Block::Paragraph(Paragraph {
                        runs: vec![Run {
                            text: "cell & <text>".to_string(),
                            ..Default::default()
                        }],
                        ..Default::default()
                    })],
                }]],
            }),
            Block::Paragraph(Paragraph {
                runs: vec![Run {
                    images: vec![ImageRef {
                        rel_id: "rIdImg".to_string(),
                        part: "word/media/image1.png".to_string(),
                        cx_emu: 9525,
                        cy_emu: 9525,
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
        ],
        headers: vec![HeaderFooter {
            part: "word/header1.xml".to_string(),
            blocks: vec![Block::Paragraph(Paragraph {
                runs: vec![Run {
                    text: "header base".to_string(),
                    ..Default::default()
                }],
                ..Default::default()
            })],
        }],
        footers: vec![],
        doc_defaults: None,
        page: None,
        page_mar: None,
        styles: BTreeMap::from([(
            "Normal".to_string(),
            StyleDef {
                kind: StyleKind::Paragraph,
                name: Some("Normal".to_string()),
                size: Some(22),
                ..Default::default()
            },
        )]),
        numbering: BTreeMap::from([(
            1u32,
            BTreeMap::from([(
                0u32,
                NumLvl {
                    num_fmt: "decimal".to_string(),
                    lvl_text: "%1.".to_string(),
                },
            )]),
        )]),
    };
    let bytes = write_docx(&base, None).unwrap();
    assert_eq!(nanooffice_ooxml::parse_docx(&bytes).unwrap(), base);

    const JUNK: &[char] = &[
        '<', '>', '&', '"', '\'', '`', '~', '@', '#', '%', '^', '*', '(', ')', '[', ']', '{', '}',
        '|', ';', ':', ',', '.', '/', '?', '\\', 'ẽ', 'ộ', 'à', '—', '•', '\t', '\n',
    ];
    let mut state: u64 = 42;
    let mut junk = |state: &mut u64| -> String {
        let mut s = String::new();
        for _ in 0..(next(state) % 6 + 1) {
            s.push(JUNK[(next(state) as usize) % JUNK.len()]);
        }
        s
    };

    for i in 0..500u32 {
        let mut m = base.clone();
        match next(&mut state) % 6 {
            // (0) junk vào text run
            0 => append_text(&mut m, &junk(&mut state)),
            // (1) numPr raw values bất kỳ
            1 => {
                for b in body_paragraphs(&mut m) {
                    if let Some(num) = &mut b.numbering {
                        num.num_id = (next(&mut state) % 100) as u32;
                        num.ilvl = (next(&mut state) % 9) as u32;
                    }
                }
            }
            // (2) image rel/part (extension luôn hợp lệ)
            2 => {
                let (id, part) = (
                    format!("rIdF{}", next(&mut state) % 100),
                    format!("word/media/img{}.png", next(&mut state) % 100),
                );
                for r in all_runs(&mut m) {
                    for img in &mut r.images {
                        img.rel_id = id.clone();
                        img.part = part.clone();
                    }
                }
            }
            // (3) style props
            3 => {
                if let Some(style) = m.styles.values_mut().next() {
                    style.bold = Some(next(&mut state).is_multiple_of(2));
                    style.size = Some((next(&mut state) % 100) as u32);
                    style.name = Some(junk(&mut state));
                }
            }
            // (4) numbering format strings bất kỳ
            4 => {
                if let Some(lvl) = m
                    .numbering
                    .values_mut()
                    .next()
                    .and_then(|l| l.values_mut().next())
                {
                    lvl.num_fmt = junk(&mut state);
                    lvl.lvl_text = junk(&mut state);
                }
            }
            // (5) paragraph alignment bất kỳ
            _ => {
                for b in body_paragraphs(&mut m) {
                    b.align = Some(junk(&mut state));
                }
            }
        }
        let bytes =
            write_docx(&m, None).unwrap_or_else(|e| panic!("fuzz round {i}: write failed: {e:?}"));
        let back = nanooffice_ooxml::parse_docx(&bytes)
            .unwrap_or_else(|e| panic!("fuzz round {i}: reparse failed: {e:?}"));
        assert_eq!(
            m, back,
            "fuzz round {i}: model changed through the round-trip"
        );
    }
}

/// LCG bước tiếp (tự viết, không dep fuzz — cùng pattern với tests/parse.rs).
fn next(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *state >> 33
}

// ---------- helpers ----------

fn empty_model() -> Document {
    Document {
        body: vec![],
        headers: vec![],
        footers: vec![],
        styles: BTreeMap::new(),
        numbering: BTreeMap::new(),
        doc_defaults: None,
        page: None,
        page_mar: None,
    }
}

fn body_paragraphs(doc: &mut Document) -> Vec<&mut Paragraph> {
    doc.body
        .iter_mut()
        .filter_map(|b| match b {
            Block::Paragraph(p) => Some(p),
            Block::Table(_) => None,
        })
        .collect()
}

/// Mọi run của body + headers + footers (cells lồng bảng).
fn all_runs(doc: &mut Document) -> Vec<&mut Run> {
    fn blocks_runs<'a>(blocks: &'a mut [Block], out: &mut Vec<&'a mut Run>) {
        for b in blocks.iter_mut() {
            match b {
                Block::Paragraph(p) => out.extend(p.runs.iter_mut()),
                Block::Table(t) => {
                    for row in &mut t.rows {
                        for cell in row.iter_mut() {
                            blocks_runs(&mut cell.blocks, out);
                        }
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    blocks_runs(&mut doc.body, &mut out);
    for hf in doc.headers.iter_mut().chain(doc.footers.iter_mut()) {
        blocks_runs(&mut hf.blocks, &mut out);
    }
    out
}

fn append_text(doc: &mut Document, junk: &str) {
    if let Some(r) = all_runs(doc).into_iter().next() {
        r.text.push_str(junk);
    }
}
