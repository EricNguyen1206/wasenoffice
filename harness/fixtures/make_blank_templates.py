#!/usr/bin/env python3
"""Generate the 3 Template blanks for Create-new (A01 T-A39, DD §5).

Usage: make_blank_templates.py [outdir]
       (default: src-tauri/resources/templates — artifacts are COMMITTED;
        run this script again only when regenerating deliberately)

Provenance (clean-room record, DD A01 §9/§10):
- blank.docx — hand-built stdlib zip following ECMA-376 / ISO 29500 Part 1
  (same style as make_docx.py; empty body = one empty paragraph + sectPr).
- blank.xlsx — openpyxl 3.1.5 (MIT) empty Workbook().
- blank.pptx — python-pptx 1.0.2 (MIT) default Presentation() — the library's
  own MIT-distributed default template, no ONLYOFFICE content involved.

The runtime consumes these through `include_bytes!` (src-tauri/src/untitled.rs)
and the gate proves the engine accepts each one via real x2t (cargo test
`untitled_templates_open_via_x2t`).
"""
import os
import sys
import zipfile

OUT_DEFAULT = os.path.join(os.path.dirname(__file__), "..", "..", "src-tauri", "resources", "templates")

# ── blank.docx (hand-built, stdlib only) ─────────────────────────────────────

DOCX_CONTENT_TYPES = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>
"""

DOCX_ROOT_RELS = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>
"""

# Empty document = one empty paragraph + section properties (a fresh Word doc).
DOCX_DOCUMENT = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p/>
    <w:sectPr>
      <w:pgSz w:w="11906" w:h="16838"/>
      <w:pgMar w:top="1417" w:right="1417" w:bottom="1417" w:left="1417" w:header="708" w:footer="708" w:gutter="0"/>
    </w:sectPr>
  </w:body>
</w:document>
"""


def write_docx(path):
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("[Content_Types].xml", DOCX_CONTENT_TYPES)
        z.writestr("_rels/.rels", DOCX_ROOT_RELS)
        z.writestr("word/document.xml", DOCX_DOCUMENT)


def main():
    outdir = sys.argv[1] if len(sys.argv) > 1 else OUT_DEFAULT
    os.makedirs(outdir, exist_ok=True)

    docx_path = os.path.join(outdir, "blank.docx")
    write_docx(docx_path)
    print(f"blank.docx  hand-built  → {docx_path}")

    import openpyxl  # MIT

    xlsx_path = os.path.join(outdir, "blank.xlsx")
    openpyxl.Workbook().save(xlsx_path)
    print(f"blank.xlsx  openpyxl    → {xlsx_path}")

    from pptx import Presentation  # python-pptx, MIT

    pptx_path = os.path.join(outdir, "blank.pptx")
    Presentation().save(pptx_path)
    print(f"blank.pptx  python-pptx → {pptx_path}")


if __name__ == "__main__":
    main()
