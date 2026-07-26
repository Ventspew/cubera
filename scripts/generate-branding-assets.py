#!/usr/bin/env python3
"""Generate polished Cubera in-game branding textures."""

from __future__ import annotations

import io
import math
import os
from pathlib import Path

import cairosvg
from PIL import Image, ImageDraw, ImageFilter, ImageFont, ImageOps

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "src-tauri" / "branding"
SVG = ROOT / "public" / "cubera.svg"


def find_font(size: int, italic: bool = False) -> ImageFont.FreeTypeFont:
    candidates = [
        "/usr/share/fonts/truetype/liberation/LiberationSerif-Italic.ttf" if italic else None,
        "/usr/share/fonts/truetype/noto/NotoSerif-Italic.ttf" if italic else None,
        "/usr/share/fonts/truetype/macos/Inter-Italic.ttf" if italic else None,
        "/usr/share/fonts/truetype/liberation/LiberationSerif-Regular.ttf" if not italic else None,
        "/usr/share/fonts/truetype/dejavu/DejaVuSerif-Bold.ttf" if not italic else None,
        "/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf" if not italic else None,
        "/usr/share/fonts/truetype/macos/Inter-SemiBold.ttf" if not italic else None,
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ]
    for path in candidates:
        if path and os.path.exists(path):
            return ImageFont.truetype(path, size=size)
    return ImageFont.load_default()


def render_svg(size: int) -> Image.Image:
    png = cairosvg.svg2png(url=str(SVG), output_width=size, output_height=size)
    return Image.open(io.BytesIO(png)).convert("RGBA")


def make_edition() -> Image.Image:
    """Classic Minecraft '* Java Edition *' energy — soft gold italic subtitle."""
    # High-res source; MC scales GUI textures cleanly from multiples of 128x16
    w, h = 512, 64
    img = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    font = find_font(42, italic=True)
    text = "* Cubera *"

    bbox = draw.textbbox((0, 0), text, font=font)
    tw, th = bbox[2] - bbox[0], bbox[3] - bbox[1]
    x = (w - tw) // 2 - bbox[0]
    y = (h - th) // 2 - bbox[1] - 2

    # Soft drop shadow like vanilla GUI text
    for ox, oy in ((2, 2), (3, 3)):
        draw.text((x + ox, y + oy), text, font=font, fill=(35, 24, 10, 160))

    # Warm cream/gold fill — close to vanilla edition yellow
    draw.text((x, y), text, font=font, fill=(255, 236, 150, 255))

    # Very subtle top highlight for depth
    highlight = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    hd = ImageDraw.Draw(highlight)
    hd.text((x, y - 1), text, font=font, fill=(255, 255, 220, 70))
    img = Image.alpha_composite(img, highlight)

    # Tiny shear for classic italic subtitle feel (font already italic; light extra)
    img = img.transform(
        (w, h),
        Image.AFFINE,
        (1, -0.08, 10, 0, 1, 0),
        resample=Image.BICUBIC,
        fillcolor=(0, 0, 0, 0),
    )
    return img


def make_pack() -> Image.Image:
    """Resource pack icon: mineral plate + copper hex monogram from brand SVG."""
    size = 256
    img = Image.new("RGBA", (size, size), (14, 11, 9, 255))
    draw = ImageDraw.Draw(img)

    # Radial copper glow
    glow = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    gd = ImageDraw.Draw(glow)
    gd.ellipse((28, 28, size - 28, size - 28), fill=(212, 137, 74, 55))
    glow = glow.filter(ImageFilter.GaussianBlur(18))
    img = Image.alpha_composite(img, glow)
    draw = ImageDraw.Draw(img)

    # Beveled frame
    draw.rounded_rectangle((6, 6, size - 7, size - 7), radius=22, outline=(232, 168, 106, 90), width=3)
    draw.rounded_rectangle((12, 12, size - 13, size - 13), radius=18, outline=(90, 60, 35, 120), width=2)

    # Brand mark
    mark = render_svg(168)
    mx = (size - mark.width) // 2
    my = 28
    img.paste(mark, (mx, my), mark)

    # Wordmark
    draw = ImageDraw.Draw(img)
    font = find_font(28, italic=False)
    text = "CUBERA"
    bbox = draw.textbbox((0, 0), text, font=font)
    tw = bbox[2] - bbox[0]
    x = (size - tw) // 2 - bbox[0]
    y = 200
    draw.text((x + 1, y + 1), text, font=font, fill=(20, 14, 8, 200))
    draw.text((x, y), text, font=font, fill=(245, 212, 168, 245))

    # Downscale to 128 for pack icon slot with high quality
    return img.resize((128, 128), Image.Resampling.LANCZOS)


def make_minceraft() -> Image.Image:
    """Rare title easter-egg — clean copper wordmark, not a second Minecraft logo."""
    w, h = 512, 128
    img = Image.new("RGBA", (w, h), (0, 0, 0, 0))

    mark = render_svg(96)
    img.paste(mark, (28, (h - mark.height) // 2), mark)

    draw = ImageDraw.Draw(img)
    font = find_font(64, italic=False)
    text = "CUBERA"
    x, y = 140, 28
    # Heavy outline like MC title weight
    for ox in range(-3, 4):
        for oy in range(-3, 4):
            if ox or oy:
                draw.text((x + ox, y + oy), text, font=font, fill=(18, 12, 8, 230))
    draw.text((x, y), text, font=font, fill=(255, 244, 220, 255))

    # Copper underline accent
    draw.rounded_rectangle((144, 100, 470, 106), radius=3, fill=(212, 137, 74, 200))
    return img


def write_splashes() -> None:
    splashes = """\
Also try underground!
As seen on title screens!
100% pure copper!
Hexagonally yours!
Now with more ore!
Technically a launcher!
Your mineral workbench!
Splashes, but classier!
Finally, a polished edge!
Watch those caves!
Built for Apple Silicon!
Not a creeper!
Shine bright like copper!
May contain diamonds!
Click me again!
Open source, closed caves!
One more block!
Portal-ready!
Crafted, not generated!
No microtransactions here!
""".strip()
    (OUT / "splashes.txt").write_text(splashes + "\n", encoding="utf-8")


def preview(edition: Image.Image, pack: Image.Image) -> None:
    """Composite a dark cave-like preview of the title treatment."""
    canvas = Image.new("RGBA", (960, 420), (18, 16, 14, 255))
    # soft vignette
    vig = Image.new("RGBA", canvas.size, (0, 0, 0, 0))
    vd = ImageDraw.Draw(vig)
    vd.ellipse((-80, -40, 1040, 520), fill=(80, 55, 30, 55))
    vig = vig.filter(ImageFilter.GaussianBlur(60))
    canvas = Image.alpha_composite(canvas, vig)

    draw = ImageDraw.Draw(canvas)
    # Fake Minecraft wordmark (stone-ish) so preview shows hierarchy
    title_font = find_font(72, italic=False)
    title = "MINECRAFT"
    bbox = draw.textbbox((0, 0), title, font=title_font)
    tw = bbox[2] - bbox[0]
    tx = (canvas.width - tw) // 2 - bbox[0]
    ty = 110
    for ox in range(-4, 5):
        for oy in range(-4, 5):
            if abs(ox) + abs(oy) > 0:
                draw.text((tx + ox, ty + oy), title, font=title_font, fill=(10, 8, 6, 255))
    draw.text((tx, ty), title, font=title_font, fill=(180, 180, 180, 255))

    ed = edition.resize((400, 50), Image.Resampling.LANCZOS)
    canvas.paste(ed, ((canvas.width - ed.width) // 2, 210), ed)

    # pack icon corner
    p = pack.resize((96, 96), Image.Resampling.NEAREST)
    canvas.paste(p, (40, 40), p)

    # splash sample
    splash_font = find_font(28, italic=True)
    splash = "Also try underground!"
    draw.text((620, 150), splash, font=splash_font, fill=(255, 255, 80, 255))

    out = Path("/opt/cursor/artifacts/cubera-ingame-branding-preview.png")
    out.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(out)
    print("preview", out)


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    edition = make_edition()
    pack = make_pack()
    egg = make_minceraft()
    edition.save(OUT / "edition.png")
    pack.save(OUT / "pack.png")
    egg.save(OUT / "title_minceraft.png")
    write_splashes()
    preview(edition, pack)
    for name in ("edition.png", "pack.png", "title_minceraft.png", "splashes.txt"):
        p = OUT / name
        print(f"{name}: {p.stat().st_size} bytes")


if __name__ == "__main__":
    main()
