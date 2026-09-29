#!/usr/bin/env python3
"""Generate a minimal-but-real XLSX fixture (stdlib only) for the multi-format
smoke gates (T-048).

Usage: make_xlsx.py [output.xlsx]   (default: /tmp/nanooffice-fixture.xlsx)
Structure follows ECMA-376 / ISO 29500 Part 1 (spreadsheetML) — no upstream
code involved. One sheet, inline strings (no sharedStrings needed), a heading
row, Vietnamese diacritics and plain numbers so both text and numeric
rendering are exercised.
"""
import sys
import zipfile

CONTENT_TYPES = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
</Types>
"""

ROOT_RELS = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>
"""

WORKBOOK_RELS = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>
"""

WORKBOOK = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
          xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Kiểm tra" sheetId="1" r:id="rId1"/>
  </sheets>
</workbook>
"""

# cell refs: A1..C4 — inlineStr for text, plain `v` for numbers
SHEET = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1">
      <c r="A1" t="inlineStr"><is><t>NanoOffice T-048 Fixture</t></is></c>
    </row>
    <row r="2">
      <c r="A2" t="inlineStr"><is><t>Tên mặt hàng</t></is></c>
      <c r="B2" t="inlineStr"><is><t>Số lượng</t></is></c>
      <c r="C2" t="inlineStr"><is><t>Đơn giá</t></is></c>
    </row>
    <row r="3">
      <c r="A3" t="inlineStr"><is><t>Bút chì chì than — đồ dùng học tập</t></is></c>
      <c r="B3"><v>12</v></c>
      <c r="C3"><v>9500.5</v></c>
    </row>
    <row r="4">
      <c r="A4" t="inlineStr"><is><t>Vở ô ly 200 trang “bìa cứng”</t></is></c>
      <c r="B4"><v>7</v></c>
      <c r="C4"><v>24500</v></c>
    </row>
  </sheetData>
</worksheet>
"""


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else "/tmp/nanooffice-fixture.xlsx"
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("[Content_Types].xml", CONTENT_TYPES)
        z.writestr("_rels/.rels", ROOT_RELS)
        z.writestr("xl/_rels/workbook.xml.rels", WORKBOOK_RELS)
        z.writestr("xl/workbook.xml", WORKBOOK)
        z.writestr("xl/worksheets/sheet1.xml", SHEET)
    print(out)


if __name__ == "__main__":
    main()
