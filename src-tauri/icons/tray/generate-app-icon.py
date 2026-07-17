"""Generates a high-res, badge-free app-icon source from mascot-source.png
(same background-strip logic as generate-tray-icons.py, minus the status
badge), for `npx tauri icon` to downscale into icon.ico/icon.icns/Square*.png.
Run from this directory: `python generate-app-icon.py`.
"""
import math
from pathlib import Path
from PIL import Image

HERE = Path(__file__).parent
OUT_SIZE = 1024
CANVAS_SCALE = 0.86


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


def main():
    mascot = extract_mascot()
    mw, mh = mascot.size
    scale = (OUT_SIZE * CANVAS_SCALE) / max(mw, mh)
    new_w, new_h = int(mw * scale), int(mh * scale)
    mascot_r = mascot.resize((new_w, new_h), Image.NEAREST)

    img = Image.new("RGBA", (OUT_SIZE, OUT_SIZE), (0, 0, 0, 0))
    ox, oy = (OUT_SIZE - new_w) // 2, (OUT_SIZE - new_h) // 2
    img.paste(mascot_r, (ox, oy), mascot_r)
    img.save(HERE / "app-icon-source.png")
    print("done ->", HERE / "app-icon-source.png")


if __name__ == "__main__":
    main()
