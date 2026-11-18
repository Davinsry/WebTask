"""Render assets/logo.svg menjadi assets/icon.ico (multi-resolusi) untuk jendela Windows.

Pakai Pillow: SVG digambar ulang secara vektor-lewat-Skia kalau ada, kalau tidak
memakai rasterisasi sederhana. Skrip dijalankan sekali saat menyiapkan aset.
"""
import os
import sys

try:
    import cairosvg
except ImportError:
    cairosvg = None

from PIL import Image


def render_svg(path: str, size: int) -> Image.Image:
    if cairosvg is not None:
        png = cairosvg.svg2png(url=path, output_width=size, output_height=size)
        from io import BytesIO

        return Image.open(BytesIO(png)).convert("RGBA")
    # Fallback: gambar ulang bentuk logo dengan Pillow (tanpa gradien).
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    from PIL import ImageDraw

    d = ImageDraw.Draw(img)
    s = size / 64.0
    body = [
        (16 * s, 8 * s), (42 * s, 8 * s), (54 * s, 20 * s), (54 * s, 50 * s),
        (54 * s, 56 * s), (10 * s, 56 * s), (10 * s, 14 * s),
    ]
    d.polygon(body, fill=(15, 23, 42, 255))
    d.polygon([(42 * s, 8 * s), (42 * s, 20 * s), (54 * s, 20 * s)], fill=(37, 99, 235, 255))
    d.line(
        [(21 * s, 35.5 * s), (28.5 * s, 43 * s), (43.5 * s, 27 * s)],
        fill=(255, 255, 255, 255),
        width=max(1, int(5.5 * s)),
        joint="curve",
    )
    return img


def encode_dib(img: Image.Image) -> bytes:
    """Encode RGBA image sebagai 32-bit BGRA DIB (BITMAPINFOHEADER + XOR + AND mask)."""
    import struct

    w, h = img.size
    rgba = img.convert("RGBA").tobytes()
    xor_rows = bytearray()
    for y in range(h - 1, -1, -1):
        row = rgba[y * w * 4 : (y + 1) * w * 4]
        for x in range(0, len(row), 4):
            r, g, b, a = row[x], row[x + 1], row[x + 2], row[x + 3]
            xor_rows.extend((b, g, r, a))

    and_stride = ((w + 31) // 32) * 4
    and_mask = b"\x00" * (and_stride * h)
    header = struct.pack(
        "<IiiHHIIiiII",
        40,              # biSize
        w,               # biWidth
        h * 2,           # biHeight (XOR + AND)
        1,               # biPlanes
        32,              # biBitCount
        0,               # biCompression (BI_RGB)
        len(xor_rows) + len(and_mask),
        0, 0, 0, 0,
    )
    return header + bytes(xor_rows) + and_mask


def save_ico(frames: list[Image.Image], path: str) -> None:
    """Simpan ICO: DIB 32-bit untuk ukuran < 256, PNG untuk 256x256."""
    import struct
    from io import BytesIO

    payloads: list[tuple[int, int, bytes]] = []
    for img in frames:
        w, h = img.size
        if w >= 256 or h >= 256:
            buf = BytesIO()
            img.save(buf, format="PNG")
            payloads.append((w, h, buf.getvalue()))
        else:
            payloads.append((w, h, encode_dib(img)))

    header = struct.pack("<HHH", 0, 1, len(payloads))
    offset = 6 + 16 * len(payloads)
    entries = bytearray()
    for w, h, data in payloads:
        entries.extend(struct.pack(
            "<BBBBHHII",
            0 if w >= 256 else w,
            0 if h >= 256 else h,
            0, 0, 1, 32, len(data), offset,
        ))
        offset += len(data)

    with open(path, "wb") as f:
        f.write(header)
        f.write(entries)
        for _, _, data in payloads:
            f.write(data)


def main() -> int:
    here = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    svg = os.path.join(here, "assets", "logo.svg")
    ico = os.path.join(here, "assets", "icon.ico")
    if not os.path.exists(svg):
        print("logo.svg tidak ditemukan:", svg)
        return 1

    sizes = [16, 24, 32, 48, 64, 128, 256]
    frames = [render_svg(svg, s) for s in sizes]
    save_ico(frames, ico)
    print("ditulis:", ico, os.path.getsize(ico), "bytes")
    return 0


if __name__ == "__main__":
    sys.exit(main())