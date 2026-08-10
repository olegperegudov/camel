"""Composes the README's menu-bar strip from the icons the app really draws.

    cd src-tauri && cargo test --lib -- --ignored dump_icons
    python3 docs/_menubar_strip.py docs/screenshots/menubar-pills.png

The RGBA dumps come from tray_icon::render itself, so the picture in the README
is the same pixels macOS gets — a hand-drawn mock would drift the first time a
colour or a shape changes here.
"""

import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
DUMP = ROOT / "src-tauri/target/icon-dump"
STATES = ["ok", "low", "update"]

W, H = 620, 64
ICON = 28
TILE = (44, 40)
BG = (27, 28, 31, 255)
TILE_FILL = (255, 255, 255, 18)
CLOCK = "14:32"


def load(name: str) -> Image.Image:
    raw = (DUMP / f"{name}.rgba").read_bytes()
    return Image.frombytes("RGBA", (32, 32), raw).resize((ICON, ICON), Image.LANCZOS)


def font(size: int) -> ImageFont.FreeTypeFont:
    for path in ("/System/Library/Fonts/SFNS.ttf", "/System/Library/Fonts/Helvetica.ttc"):
        if Path(path).exists():
            return ImageFont.truetype(path, size)
    return ImageFont.load_default()


def main(out: Path) -> None:
    strip = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    draw = ImageDraw.Draw(strip)
    draw.rounded_rectangle((0, 0, W - 1, H - 1), radius=14, fill=BG)

    # Icons on the left, clock on the right — the same order macOS gives them.
    # Tiles go through their own layer: ImageDraw writes pixels rather than
    # blending them, so a translucent fill drawn straight on would come out
    # opaque white.
    tiles = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    tile_draw = ImageDraw.Draw(tiles)
    x = 34
    for name in STATES:
        cx, cy = x + ICON // 2, H // 2
        tile_draw.rounded_rectangle(
            (cx - TILE[0] // 2, cy - TILE[1] // 2, cx + TILE[0] // 2, cy + TILE[1] // 2),
            radius=9,
            fill=TILE_FILL,
        )
        x += 168
    strip.alpha_composite(tiles)

    x = 34
    for name in STATES:
        strip.alpha_composite(load(name), (x, H // 2 - ICON // 2))
        x += 168

    draw.text((W - 60, H // 2), CLOCK, font=font(20), fill=(236, 236, 240, 255), anchor="mm")
    strip.save(out)
    print(out)


if __name__ == "__main__":
    main(Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "docs/screenshots/menubar-pills.png")
