#!/usr/bin/env python3
"""Generate a minimal-but-real PDF fixture (stdlib only, handwritten per
ISO 32000) for the multi-format smoke gates (T-048).

Usage: make_pdf.py [output.pdf]   (default: /tmp/wasenoffice-fixture.pdf)
One page, base-14 Helvetica text, xref table computed from real byte offsets.
Vietnamese diacritics live in the document Title metadata (UTF-16BE hex
string): the base-14 fonts are WinAnsi-encoded and cannot render Vietnamese
glyphs, and embedding a TrueType font in a hand-written fixture is out of
proportion for a smoke fixture. Body text stays ASCII on purpose.
"""
import sys

TEXT_LINES = [
    "WasenOffice T-048 Fixture",
    "If this page renders in the WKWebView canvas,",
    "the PDF editor is alive inside the Tauri shell.",
]


def utf16be_hex(s: str) -> str:
    return ("FEFF" + s.encode("utf-16-be").hex().upper())


def build_pdf() -> bytes:
    title_utf16 = utf16be_hex("Tệp kiểm tra WasenOffice — T-048")

    info = f"<< /Title <{title_utf16}> /Producer (WasenOffice fixture) >>"
    content_lines = "\n".join(
        f"BT /F1 18 Tf 72 {740 - i * 32} Td ({line}) Tj ET" for i, line in enumerate(TEXT_LINES)
    )
    content = f"q 1 0 0 1 0 0 cm\n{content_lines}\nQ"

    # (num, generation, byte-offset) filled during assembly. Object bytes don't
    # depend on offsets (offsets only widen to fixed-width xref entries), so a
    # single pass is already self-consistent.
    objects = [
        "<< /Type /Catalog /Pages 2 0 R >>",                                   # 1
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",                           # 2
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] "
        "/Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>",           # 3
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",              # 4
        f"<< /Length {len(content.encode('utf-8'))} >>\nstream\n{content}\nendstream",  # 5
        info,                                                                  # 6
    ]

    def assemble(offsets):
        buf = bytearray(b"%PDF-1.4\n%\xc7\xec\x8f\xa2\n")
        for i, obj in enumerate(objects, start=1):
            offsets[i] = len(buf)
            buf += f"{i} 0 obj\n{obj}\nendobj\n".encode("utf-8")
        xref_pos = len(buf)
        buf += f"xref\n0 {len(objects) + 1}\n".encode("utf-8")
        buf += b"0000000000 65535 f \n"
        for i in range(1, len(objects) + 1):
            buf += f"{offsets[i]:010d} 00000 n \n".encode("utf-8")
        buf += (
            f"trailer\n<< /Size {len(objects) + 1} /Root 1 0 R /Info 6 0 R >>\n"
            f"startxref\n{xref_pos}\n%%EOF\n"
        ).encode("utf-8")
        return bytes(buf)

    # offsets don't influence object bytes → one pass is already consistent
    return assemble({})


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else "/tmp/wasenoffice-fixture.pdf"
    data = build_pdf()
    with open(out, "wb") as f:
        f.write(data)
    print(out)


if __name__ == "__main__":
    main()
