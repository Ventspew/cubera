#!/usr/bin/env python3
"""Generate Cubera in-game branding textures (edition logo lockup, pack icon, egg)."""

from __future__ import annotations

import io
import urllib.request
from pathlib import Path

import cairosvg
from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "src-tauri" / "branding"
SVG = ROOT / "public" / "cubera.svg"
ASCII_URL = (
    "https://raw.githubusercontent.com/InventivetalentDev/minecraft-assets/"
    "1.21.4/assets/minecraft/textures/font/ascii.png"
)
ASCII_CACHE = Path("/tmp/mc-ascii.png")

FACE_COPPER = (245, 212, 168, 255)
SHADOW = (40, 24, 12, 230)


def load_ascii() -> Image.Image:
    if not ASCII_CACHE.exists():
        data = urllib.request.urlopen(ASCII_URL, timeout=30).read()
        ASCII_CACHE.write_bytes(data)
    return Image.open(ASCII_CACHE).convert("RGBA")


def render_svg(size: int) -> Image.Image:
    png = cairosvg.svg2png(url=str(SVG), output_width=size, output_height=size)
    return Image.open(io.BytesIO(png)).convert("RGBA")


def glyph(ascii_img: Image.Image, ch: str) -> tuple[Image.Image, int]:
    cell = ascii_img.width // 16
    code = ord(ch)
    col, row = code % 16, code // 16
    g = ascii_img.crop((col * cell, row * cell, (col + 1) * cell, (row + 1) * cell))
    px = g.load()
    max_x = 0
    for y in range(g.height):
        for x in range(g.width):
            if px[x, y][3] > 0:
                max_x = max(max_x, x)
    return g, max(1, max_x + 1)


def render_mc_text(
    ascii_img: Image.Image,
    text: str,
    scale: int,
    face=FACE_COPPER,
    shadow=SHADOW,
) -> Image.Image:
    cell = ascii_img.width // 16
    glyphs = []
    total_w = 0
    for ch in text:
        g, w = glyph(ascii_img, ch)
        glyphs.append((g, w))
        total_w += w + 1
    total_w = max(1, total_w - 1)
    base = Image.new("RGBA", (total_w, cell), (0, 0, 0, 0))
    x = 0
    for g, w in glyphs:
        base.paste(g, (x, 0), g)
        x += w + 1
    base = base.resize((total_w * scale, cell * scale), Image.NEAREST)

    out = Image.new("RGBA", (base.width + scale, base.height + scale), (0, 0, 0, 0))
    sh = Image.new("RGBA", out.size, (0, 0, 0, 0))
    fc = Image.new("RGBA", out.size, (0, 0, 0, 0))
    bp, sp, fp = base.load(), sh.load(), fc.load()
    for y in range(base.height):
        for x in range(base.width):
            if bp[x, y][3] > 0:
                sp[x + scale, y + scale] = shadow
                fp[x, y] = face
    return Image.alpha_composite(Image.alpha_composite(out, sh), fc)


def make_edition(ascii_img: Image.Image) -> Image.Image:
    """Under-title slot: Cubera hex logo + CUBERA wordmark (replaces Java Edition)."""
    w, h = 512, 64
    canvas = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    mark = render_svg(56)
    word = render_mc_text(ascii_img, "CUBERA", scale=3)
    gap = 12
    total = mark.width + gap + word.width
    x0 = (w - total) // 2
    canvas.paste(mark, (x0, (h - mark.height) // 2), mark)
    canvas.paste(word, (x0 + mark.width + gap, (h - word.height) // 2 + 1), word)

    glow = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    cx, cy = w // 2, h // 2
    for y in range(h):
        for x in range(max(0, x0 - 6), min(w, x0 + total + 6)):
            dx = (x - cx) / (total * 0.55)
            dy = (y - cy) / 18
            a = int(55 * max(0.0, 1.0 - (dx * dx + dy * dy)))
            if a > 0:
                glow.putpixel((x, y), (120, 70, 35, a))
    glow = glow.filter(ImageFilter.GaussianBlur(3))
    return Image.alpha_composite(glow, canvas)


def make_pack(ascii_img: Image.Image) -> Image.Image:
    size = 256
    img = Image.new("RGBA", (size, size), (12, 10, 8, 255))
    glow = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    ImageDraw.Draw(glow).ellipse((20, 20, size - 20, size - 20), fill=(212, 137, 74, 70))
    img = Image.alpha_composite(img, glow.filter(ImageFilter.GaussianBlur(18)))
    draw = ImageDraw.Draw(img)
    draw.rounded_rectangle(
        (5, 5, size - 6, size - 6), radius=24, outline=(232, 168, 106, 120), width=3
    )
    mark = render_svg(168)
    img.paste(mark, ((size - mark.width) // 2, 18), mark)
    word = render_mc_text(ascii_img, "CUBERA", scale=2)
    img.paste(word, ((size - word.width) // 2, 202), word)
    return img.resize((128, 128), Image.Resampling.LANCZOS)


def make_egg(ascii_img: Image.Image) -> Image.Image:
    w, h = 512, 128
    canvas = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    mark = render_svg(100)
    word = render_mc_text(
        ascii_img, "CUBERA", scale=5, face=(255, 236, 210, 255), shadow=(20, 12, 8, 255)
    )
    total = mark.width + 18 + word.width
    x0 = (w - total) // 2
    canvas.paste(mark, (x0, (h - mark.height) // 2), mark)
    canvas.paste(word, (x0 + mark.width + 18, (h - word.height) // 2), word)
    return canvas


def write_splashes() -> None:
    (OUT / "splashes.txt").write_text(
        "\n".join(
            [
                "Also try underground!",
                "As seen on title screens!",
                "100% pure copper!",
                "Hexagonally yours!",
                "Now with more ore!",
                "Technically a launcher!",
                "Your mineral workbench!",
                "Finally, a polished edge!",
                "Watch those caves!",
                "Built for Apple Silicon!",
                "Not a creeper!",
                "Shine bright like copper!",
                "May contain diamonds!",
                "Click me again!",
                "Open source, closed caves!",
                "One more block!",
                "Portal-ready!",
                "Crafted, not generated!",
                "No microtransactions here!",
            ]
        )
        + "\n",
        encoding="utf-8",
    )


def preview(edition: Image.Image, pack: Image.Image) -> None:
    canvas = Image.new("RGBA", (960, 420), (20, 16, 14, 255))
    for y in range(420):
        for x in range(960):
            v = int(18 + 10 * ((x - 480) ** 2 + (y - 210) ** 2) ** 0.5 / 600)
            canvas.putpixel((x, y), (min(40, v + 8), min(32, v + 4), min(28, v), 255))
    draw = ImageDraw.Draw(canvas)
    try:
        font = ImageFont.truetype(
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", 72
        )
    except OSError:
        font = ImageFont.load_default()
    title = "MINECRAFT"
    bbox = draw.textbbox((0, 0), title, font=font)
    tx = (960 - (bbox[2] - bbox[0])) // 2
    for ox in range(-4, 5):
        for oy in range(-4, 5):
            if ox or oy:
                draw.text((tx + ox, 95 + oy), title, font=font, fill=(5, 5, 5, 255))
    draw.text((tx, 95), title, font=font, fill=(175, 175, 175, 255))
    canvas.paste(edition, ((960 - edition.width) // 2, 195), edition)
    p = pack.resize((96, 96), Image.NEAREST)
    canvas.paste(p, (32, 32), p)
    out = Path("/opt/cursor/artifacts/cubera-ingame-branding-preview.png")
    out.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(out)
    print("preview", out)


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    ascii_img = load_ascii()
    edition = make_edition(ascii_img)
    pack = make_pack(ascii_img)
    egg = make_egg(ascii_img)
    edition.save(OUT / "edition.png")
    pack.save(OUT / "pack.png")
    egg.save(OUT / "title_minceraft.png")
    write_splashes()
    preview(edition, pack)
    for name in ("edition.png", "pack.png", "title_minceraft.png", "splashes.txt"):
        print(f"{name}: {(OUT / name).stat().st_size} bytes")


if __name__ == "__main__":
    main()
