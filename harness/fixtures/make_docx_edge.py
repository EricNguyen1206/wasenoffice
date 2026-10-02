#!/usr/bin/env python3
"""Generate the edge-corpus DOCX fixtures for T-A23 (A2/S3 parity harness).

Usage: make_docx_edge.py [outdir]
  (default: harness/fixtures/corpus-a2)

Four fixtures that probe constructs the base fixtures (make_docx.py,
make_docx_full.py) do not exercise, spec-derived from ECMA-376 / ISO 29500
(stdlib only, no upstream code). Each file has an expected-model note in
`corpus-a2/NOTES.md` and here in its docstring:

  edge-multilevel-list.docx  — one abstractNum, 3 levels (decimal / lowerRoman
                               / lowerLetter), paragraphs at ilvl 0..2.
  edge-nested-table.docx     — outer 1x2 table whose second cell holds a
                               nested 2x1 table + a trailing paragraph.
  edge-style-override.docx   — style chain Normal -> Quote -> IntenseQuote
                               (2 basedOn hops) + character style Strong +
                               explicit run overrides (italic=false, bold=false).
  edge-image-multi.docx      — one image relationship referenced by 3 drawings
                               with different extents.

All generators share the minimal-package writer of make_docx_full.py.
"""
import struct
import sys
import zipfile
import zlib

W = 'xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"'
R = 'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"'
WP = 'xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"'
A = 'xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"'
PIC = 'xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"'
ROOT = f"{W} {R} {WP} {A} {PIC}"


def png_1x1() -> bytes:
    def chunk(tag: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + tag + data + struct.pack(
            ">I", zlib.crc32(tag + data)
        )
    ihdr = struct.pack(">IIBBBBB", 1, 1, 8, 2, 0, 0, 0)
    idat = zlib.compress(b"\x00\x00\x00\xff")  # filter byte + RGB(0,0,255)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", idat) + chunk(b"IEND", b"")


def package(parts: dict[str, str | bytes]) -> None:
    out = sys.argv[1] if len(sys.argv) > 1 else "harness/fixtures/corpus-a2"
    name = parts.pop("__name__")
    with zipfile.ZipFile(f"{out}/{name}", "w", zipfile.ZIP_DEFLATED) as z:
        for part, content in parts.items():
            z.writestr(part, content)


def content_types(extra_overrides: str) -> str:
    return f"""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>{extra_overrides}
</Types>
"""


ROOT_RELS = f"""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>
"""


def doc_rels(extra: str) -> str:
    return f"""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>{extra}
</Relationships>
"""


def document(body: str) -> str:
    return (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f'<w:document {ROOT}><w:body>{body}</w:body></w:document>'
    )


# ---------------------------------------------------------------- fixtures

def make_multilevel_list() -> None:
    """Expected model: numbering {1: {0: decimal '%1.', 1: lowerRoman '%2.',
    2: lowerLetter '%3)'}}; 5 paragraphs — ilvl 0, 0, 1, 1, 2 all numId=1."""
    numbering = f"""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:numbering {W}>
  <w:abstractNum w:abstractNumId="0">
    <w:multiLevelType w:val="multilevel"/>
    <w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/></w:lvl>
    <w:lvl w:ilvl="1"><w:start w:val="1"/><w:numFmt w:val="lowerRoman"/><w:lvlText w:val="%2."/></w:lvl>
    <w:lvl w:ilvl="2"><w:start w:val="1"/><w:numFmt w:val="lowerLetter"/><w:lvlText w:val="%3)"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>
</w:numbering>
"""
    body = "".join(
        f'<w:p><w:pPr><w:numPr><w:ilvl w:val="{lvl}"/><w:numId w:val="1"/></w:numPr></w:pPr>'
        f'<w:r><w:t xml:space="preserve">{text}</w:t></w:r></w:p>'
        for lvl, text in [
            (0, "Chương: Kiến trúc NanoOffice"),
            (0, "Chương: Ràng buộc license"),
            (1, "Mục con: shell Rust"),
            (1, "Mục con: engine giữ nguyên"),
            (2, "Kết: clean-room từ spec"),
        ]
    )
    package({
        "__name__": "edge-multilevel-list.docx",
        "[Content_Types].xml": content_types(
            '<Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/>'
        ),
        "_rels/.rels": ROOT_RELS,
        "word/_rels/document.xml.rels": doc_rels(
            '<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/>'
        ),
        "word/styles.xml": f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles {W}/>',
        "word/numbering.xml": numbering,
        "word/document.xml": document(body),
    })


def make_nested_table() -> None:
    """Expected model: body = [para 'Bảng lồng:', table 1x2 — cell(0,0) = para
    'trống', cell(0,1) = [table 2x1 ('ô trong', 'hàng hai'), para 'sau bảng trong'],
    para 'Kết thúc bảng lồng.']."""
    inner = (
        "<w:tbl><w:tr>"
        '<w:tc><w:p><w:r><w:t>ô trong</w:t></w:r></w:p></w:tc>'
        "</w:tr><w:tr>"
        '<w:tc><w:p><w:r><w:t>hàng hai</w:t></w:r></w:p></w:tc>'
        "</w:tr></w:tbl>"
        '<w:p><w:r><w:t xml:space="preserve">sau bảng trong</w:t></w:r></w:p>'
    )
    body = (
        '<w:p><w:r><w:t xml:space="preserve">Bảng lồng:</w:t></w:r></w:p>'
        "<w:tbl><w:tr>"
        '<w:tc><w:p><w:r><w:t>trống</w:t></w:r></w:p></w:tc>'
        f'<w:tc>{inner}</w:tc>'
        "</w:tr></w:tbl>"
        '<w:p><w:r><w:t xml:space="preserve">Kết thúc bảng lồng.</w:t></w:r></w:p>'
    )
    package({
        "__name__": "edge-nested-table.docx",
        "[Content_Types].xml": content_types(""),
        "_rels/.rels": ROOT_RELS,
        "word/_rels/document.xml.rels": doc_rels(""),
        "word/styles.xml": f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles {W}/>',
        "word/document.xml": document(body),
    })


def make_style_override() -> None:
    """Expected model: styles — IntenseQuote {bold Some(true), italic
    Some(true)} (2 basedOn hops), Quote {italic Some(true)}, Normal {}, Strong
    (character) {bold Some(true)}; runs — run trên Quote có italic=false
    override, run trên IntenseQuote có bold=false override, 1 run rStyle
    Strong."""
    styles = f"""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles {W}>
  <w:style w:type="paragraph" w:default="1" w:styleId="Normal">
    <w:name w:val="Normal"/>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Quote">
    <w:name w:val="Quote"/>
    <w:basedOn w:val="Normal"/>
    <w:rPr><w:i/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="IntenseQuote">
    <w:name w:val="Intense Quote"/>
    <w:basedOn w:val="Quote"/>
    <w:rPr><w:b/></w:rPr>
  </w:style>
  <w:style w:type="character" w:styleId="Strong">
    <w:name w:val="Strong"/>
    <w:rPr><w:b/></w:rPr>
  </w:style>
</w:styles>
"""
    body = (
        '<w:p><w:pPr><w:pStyle w:val="Quote"/></w:pPr>'
        '<w:r><w:rPr><w:i w:val="false"/></w:rPr>'
        '<w:t>Trích dẫn tắt nghiêng — run này tắt nghiêng.</w:t></w:r></w:p>'
        '<w:p><w:pPr><w:pStyle w:val="IntenseQuote"/></w:pPr>'
        '<w:r><w:rPr><w:b w:val="false"/></w:rPr>'
        '<w:t>Trích dẫn mạnh — run này tắt bold.</w:t></w:r></w:p>'
        '<w:p><w:r><w:rPr><w:rStyle w:val="Strong"/></w:rPr>'
        '<w:t>Run dùng character style Strong.</w:t></w:r></w:p>'
    )
    package({
        "__name__": "edge-style-override.docx",
        "[Content_Types].xml": content_types(""),
        "_rels/.rels": ROOT_RELS,
        "word/_rels/document.xml.rels": doc_rels(""),
        "word/styles.xml": styles,
        "word/document.xml": document(body),
    })


def make_image_multi() -> None:
    """Expected model: 3 paragraphs, mỗi run 1 image — CÙNG rel rIdImg, part
    word/media/image1.png, extents 9525x9525 / 19050x9525 / 28575x9525 EMU."""
    def drawing(cx: int, cy: int, pic_id: int, name: str) -> str:
        return (
            f'<w:r><w:drawing><wp:inline><wp:extent cx="{cx}" cy="{cy}"/>'
            f'<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">'
            f'<pic:pic><pic:nvPicPr><pic:cNvPr id="{pic_id}" name="{name}"/><pic:cNvPicPr/></pic:nvPicPr>'
            f'<pic:blipFill><a:blip r:embed="rIdImg"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>'
            f'<pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm>'
            f'<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr>'
            f'</pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>'
        )

    body = (
        f'<w:p>{drawing(9525, 9525, 1, "nho")}</w:p>'
        f'<w:p>{drawing(19050, 9525, 2, "rộng")}</w:p>'
        f'<w:p>{drawing(28575, 9525, 3, "rộng-hơn")}</w:p>'
    )
    package({
        "__name__": "edge-image-multi.docx",
        "[Content_Types].xml": content_types(""),
        "_rels/.rels": ROOT_RELS,
        "word/_rels/document.xml.rels": doc_rels(
            '<Relationship Id="rIdImg" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>'
        ),
        "word/styles.xml": f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles {W}/>',
        "word/media/image1.png": png_1x1(),
        "word/document.xml": document(body),
    })


if __name__ == "__main__":
    make_multilevel_list()
    make_nested_table()
    make_style_override()
    make_image_multi()
    out = sys.argv[1] if len(sys.argv) > 1 else "harness/fixtures/corpus-a2"
    print(out)
