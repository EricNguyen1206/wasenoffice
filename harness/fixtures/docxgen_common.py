#!/usr/bin/env python3
"""Shared helpers for the DOCX fixture generators (stdlib only).

T-056 (review #3 C5): png_1x1()+chunk() were copy-pasted across generators —
one home here. Namespace constants stay per-generator (each binds what it
uses); import from this module when a new generator needs the png helpers.
"""
import struct
import zlib


def chunk(tag: bytes, data: bytes) -> bytes:
    return struct.pack(">I", len(data)) + tag + data + struct.pack(
        ">I", zlib.crc32(tag + data)
    )


def png(rgb_rows: list[list[tuple[int, int, int]]]) -> bytes:
    """Truecolor PNG, one filter-0 scanline per row."""
    raw = b"".join(
        b"\x00" + b"".join(struct.pack("BBB", *px) for px in row) for row in rgb_rows
    )
    ihdr = struct.pack(">IIBBBBB", len(rgb_rows[0]), len(rgb_rows), 8, 2, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b"")


def png_1x1(r: int, g: int, b: int) -> bytes:
    return png([[(r, g, b)]])
