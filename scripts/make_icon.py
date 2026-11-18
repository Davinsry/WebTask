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


def main() -> int:
    here = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    svg = os.path.join(here, "assets", "logo.svg")
    ico = os.path.join(here, "assets", "icon.ico")
    if not os.path.exists(svg):
        print("logo.svg tidak ditemukan:", svg)
        return 1

    sizes = [16, 24, 32, 48, 64, 128, 256]
    frames = [render_svg(svg, s) for s in sizes]
    frames[-1].save(ico, format="ICO", sizes=[(s, s) for s in sizes])
    print("ditulis:", ico, os.path.getsize(ico), "bytes")
    return 0


if __name__ == "__main__":
    sys.exit(main())