#!/usr/bin/env python3
"""Generate the media fixture DOCX for T-A26 (canonical keeps real image bytes).

The corpus so far carried only 1×1 PNGs, so the writer's placeholder swap was
invisible to every gate. This fixture carries two DISTINCT multi-pixel PNGs
(stdlib struct+zlib, colors unlike the 1×1 placeholder), one in the body and
one in the header — the header drawing also covers the write.rs header-image
branch (review #3 C5 coverage hole). Spec-derived from ECMA-376, no upstream
code involved.

Usage: make_docx_media.py [output.docx]
  (default: src-tauri/crates/nanooffice-ooxml/tests/fixtures/nanooffice-media-fixture.docx —
   the crate tests/fixtures dir is globbed into the parity corpus by parity-a2.sh)
"""
import sys
import zipfile

from docxgen_common import png

W = 'xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"'
R = 'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"'
WP = 'xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"'
A = 'xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"'
PIC = 'xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"'
ROOT = f"{W} {R} {WP} {A} {PIC}"


# Body 2×2 and header 3×1: distinct sizes AND colors — byte comparison in the
# regression test must never confuse one with the other or with the 1×1
# placeholder (white).
BODY_PNG = png([[(0xFF, 0x00, 0x4C), (0x00, 0xB4, 0x2B)], [(0x41, 0x1E, 0xF0), (0xFF, 0xD5, 0x00)]])
HEADER_PNG = png([[(0x00, 0x00, 0x00), (0x00, 0xFF, 0xFF), (0xFF, 0x7F, 0x50)]])

CONTENT_TYPES = f"""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/>
</Types>
"""

ROOT_RELS = f"""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>
"""

DOC_RELS = f"""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdImg" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>
  <Relationship Id="rIdHead" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/>
</Relationships>
"""

HEADER_RELS = f"""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdHImg" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image2.png"/>
</Relationships>
"""


def drawing(rel_id: str, cx: int, cy: int, pic_id: int) -> str:
    return (
        '<w:r><w:drawing>'
        f'<wp:inline><wp:extent cx="{cx}" cy="{cy}"/>'
        f'<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">'
        f'<pic:pic><pic:nvPicPr><pic:cNvPr id="{pic_id}" name="px{pic_id}"/><pic:cNvPicPr/></pic:nvPicPr>'
        f'<pic:blipFill><a:blip r:embed="{rel_id}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>'
        f'<pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm>'
        f'<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr>'
        f'</pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>'
    )


DOCUMENT = (
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    f'<w:document {ROOT}><w:body>'
    '<w:p><w:r><w:t xml:space="preserve">Ảnh 2×2 giữ nguyên bytes gốc:</w:t></w:r></w:p>'
    f'<w:p>{drawing("rIdImg", 19050, 19050, 1)}</w:p>'
    '<w:p><w:r><w:t xml:space="preserve">Ếnghệ ộ lại là là — media T-A26.</w:t></w:r></w:p>'
    # header activated through sectPr (as real files do) — an unreferenced
    # header part is legal OPC but the engine drops it (oracle-obs, parity)
    '<w:sectPr><w:headerReference w:type="default" r:id="rIdHead"/></w:sectPr>'
    '</w:body></w:document>'
)

HEADER1 = (
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    f'<w:hdr {W}><w:p>{drawing("rIdHImg", 28575, 9525, 1)}'
    '<w:r><w:t xml:space="preserve">header 3×1</w:t></w:r></w:p></w:hdr>'
)


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else (
        "src-tauri/crates/nanooffice-ooxml/tests/fixtures/nanooffice-media-fixture.docx"
    )
    parts = {
        "[Content_Types].xml": CONTENT_TYPES,
        "_rels/.rels": ROOT_RELS,
        "word/_rels/document.xml.rels": DOC_RELS,
        "word/_rels/header1.xml.rels": HEADER_RELS,
        "word/media/image1.png": BODY_PNG,
        "word/media/image2.png": HEADER_PNG,
        "word/header1.xml": HEADER1,
        "word/document.xml": DOCUMENT,
    }
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        for part, content in parts.items():
            # pinned timestamp → regenerate-and-cmp detects generator↔corpus drift
            zi = zipfile.ZipInfo(part, date_time=(2026, 10, 3, 0, 0, 0))
            zi.compress_type = zipfile.ZIP_DEFLATED
            z.writestr(zi, content)
    print(out)


if __name__ == "__main__":
    main()
