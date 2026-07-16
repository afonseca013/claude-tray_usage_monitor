"""Generates the tray-icon PNGs from mascot-source.png: strips the flat
background color to transparency, then stamps a small status-colored badge
(circle) in the corner. Run from this directory: `python generate-tray-icons.py`.
"""
import math
from pathlib import Path
from PIL import Image, ImageDraw

HERE = Path(__file__).parent
SIZE = 64
CANVAS_SCALE = 0.86

COLORS = {
    "ok": (52, 211, 153, 255),
    "warning": (251, 191, 36, 255),
    "rejected": (248, 113, 113, 255),
    "unavailable": (85, 85, 95, 255),
    "error": (248, 113, 113, 255),
}


def extract_mascot() -> Image.Image:
    src = Image.open(HERE / "mascot-source.png").convert("RGB")
    bg = src.getpixel((0, 0))
    w, h = src.size
    out = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    for y in range(h):
        for x in range(w):
            p = src.getpixel((x, y))
            if math.dist(p, bg) < 30:
                continue
            out.putpixel((x, y), (*p, 255))
    return out.crop(out.getbbox())


def make(mascot: Image.Image, path: Path, color, dim: bool = False):
    mw, mh = mascot.size
    scale = (SIZE * CANVAS_SCALE) / max(mw, mh)
    new_w, new_h = int(mw * scale), int(mh * scale)
    mascot_r = mascot.resize((new_w, new_h), Image.NEAREST)

    img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    ox, oy = (SIZE - new_w) // 2, (SIZE - new_h) // 2 - 3
    img.paste(mascot_r, (ox, oy), mascot_r)

    d = ImageDraw.Draw(img)
    r = 12
    cx, cy = SIZE - r - 2, SIZE - r - 2
    badge = color if not dim else (*color[:3], 90)
    d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=badge, outline=(23, 23, 27, 255), width=3)
    img.save(path)


def main():
    mascot = extract_mascot()
    make(mascot, HERE / "ok.png", COLORS["ok"])
    make(mascot, HERE / "warning.png", COLORS["warning"])
    make(mascot, HERE / "rejected.png", COLORS["rejected"])
    make(mascot, HERE / "rejected_dim.png", COLORS["rejected"], dim=True)
    make(mascot, HERE / "unavailable.png", COLORS["unavailable"])
    make(mascot, HERE / "error.png", COLORS["error"])
    print("done")


if __name__ == "__main__":
    main()
