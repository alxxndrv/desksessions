# /// script
# requires-python = ">=3.11"
# dependencies = ["freetype-py>=2.5", "fonttools>=4.50"]
# ///
"""Renders TrueType fonts into the anti-aliased VLW format the renderer reads.

    uv run tools/make_vlw.py

Writes crates/ui/fonts/*.vlw for the sizes and weights listed in FONTS below; the ui crate
embeds them with include_bytes!. The format is LovyanGFX's, so the glyphs are the ones the C++
firmware drew.
VLW layout (as read by lgfx::VLWfont::loadFont), all integers 32-bit big-endian:
  header: glyph count, version, size, 0, ascent, descent
  per glyph, sorted by code point: unicode, height, width, xAdvance, dY (top above baseline),
                                   dX (left offset), 0
  then each glyph's width × height alpha bitmap, in the same order.
"""

import io
import struct
from pathlib import Path

import freetype
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "fonts" / "GolosText[wght].ttf"
OUT = ROOT / "crates" / "ui" / "fonts"

TEXT = (
    list(range(0x20, 0x7F))            # ASCII
    + list(range(0xA0, 0x100))         # Latin-1
    + list(range(0x100, 0x180))        # Latin Extended-A
    + list(range(0x400, 0x460))        # Cyrillic
    + [0x2013, 0x2014, 0x2018, 0x2019, 0x201C, 0x201D, 0x201E, 0x2022, 0x2026, 0x2116,
       0x00D7, 0x2190, 0x2191, 0x2192, 0x2193, 0x2039, 0x203A]
)
DIGITS = [0x20] + list(range(0x30, 0x3A))

# name, pixel size, weight, characters
FONTS = [
    ("golos-16-medium", 16, 500, TEXT),
    ("golos-12-regular", 12, 400, TEXT),
    ("golos-12-medium", 12, 500, TEXT),
    ("golos-34-semibold", 34, 600, DIGITS),
]


def instance(weight: int) -> bytes:
    font = TTFont(SOURCE)
    static = instancer.instantiateVariableFont(font, {"wght": weight})
    buffer = io.BytesIO()
    static.save(buffer)
    return buffer.getvalue()


def render(data: bytes, size: int, codes: list[int]) -> bytes:
    face = freetype.Face(io.BytesIO(data))
    face.set_pixel_sizes(0, size)
    flags = freetype.FT_LOAD_RENDER | freetype.FT_LOAD_TARGET_LIGHT

    glyphs = []
    for code in sorted(set(codes)):
        if face.get_char_index(code) == 0:
            continue  # not in the font: LovyanGFX draws a gap
        face.load_char(chr(code), flags)
        g = face.glyph
        bmp = g.bitmap
        width, height = bmp.width, bmp.rows
        pixels = bytearray()
        for row in range(height):
            start = row * bmp.pitch
            pixels += bytes(bmp.buffer[start:start + width])
        advance = round(g.advance.x / 64)
        glyphs.append((code, height, width, advance, g.bitmap_top, g.bitmap_left, bytes(pixels)))

    metrics = face.size
    ascent = round(metrics.ascender / 64)
    descent = round(-metrics.descender / 64)
    out = bytearray(struct.pack(">6i", len(glyphs), 11, size, 0, ascent, descent))
    for code, height, width, advance, top, left, _ in glyphs:
        out += struct.pack(">7i", code, height, width, advance, top, left, 0)
    for *_, pixels in glyphs:
        out += pixels
    return bytes(out)


def main() -> None:
    cache: dict[int, bytes] = {}
    for name, size, weight, codes in FONTS:
        data = cache.setdefault(weight, instance(weight))
        vlw = render(data, size, codes)
        (OUT / f"{name}.vlw").write_bytes(vlw)
        print(f"{name}: {struct.unpack('>i', vlw[:4])[0]} glyphs, {len(vlw):,} bytes")


if __name__ == "__main__":
    main()
