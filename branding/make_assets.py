"""Generate Vector branding assets from the airship artwork.

Removes the flat off-white background (edge flood fill + colour decontamination of the
anti-aliased rim), then writes:
  crates/vector-app/assets/airship.png      transparent master, cropped with padding
  crates/vector-app/assets/icon-256.png     square app icon (window icon)
  crates/vector-app/assets/vector.ico       multi-size Windows icon (16..256)
  installer/wizard-large.bmp, wizard-small.bmp   Inno Setup wizard images
Usage: python make_assets.py <source image>
"""
import sys
from collections import deque
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "crates" / "vector-app" / "assets"
INSTALLER = ROOT / "installer"


def remove_background(im: Image.Image) -> Image.Image:
    im = im.convert("RGB")
    w, h = im.size
    px = im.load()
    bg = px[2, 2]

    def diff(c):
        return max(abs(c[0] - bg[0]), abs(c[1] - bg[1]), abs(c[2] - bg[2]))

    # 1. flood fill the background from the borders (tolerant to JPEG/WebP noise)
    is_bg = bytearray(w * h)
    q = deque()
    for x in range(w):
        q.append((x, 0))
        q.append((x, h - 1))
    for y in range(h):
        q.append((0, y))
        q.append((w - 1, y))
    while q:
        x, y = q.popleft()
        i = y * w + x
        if is_bg[i]:
            continue
        if diff(px[x, y]) > 3:
            continue
        is_bg[i] = 1
        if x > 0:
            q.append((x - 1, y))
        if x < w - 1:
            q.append((x + 1, y))
        if y > 0:
            q.append((x, y - 1))
        if y < h - 1:
            q.append((x, y + 1))

    # 2. rim pixels (foreground touching background) get alpha from colour unmixing
    out = Image.new("RGBA", (w, h))
    op = out.load()
    R = 3

    def near_bg(x, y):
        for dy in range(-1, 2):
            for dx in range(-1, 2):
                xx, yy = x + dx, y + dy
                if 0 <= xx < w and 0 <= yy < h and is_bg[yy * w + xx]:
                    return True
        return False

    rim = bytearray(w * h)
    for y in range(h):
        for x in range(w):
            if not is_bg[y * w + x] and near_bg(x, y):
                rim[y * w + x] = 1
    # widen the rim by one pixel inward so the full AA ramp is covered
    rim2 = bytearray(rim)
    for y in range(1, h - 1):
        for x in range(1, w - 1):
            if rim[y * w + x]:
                for dy in (-1, 0, 1):
                    for dx in (-1, 0, 1):
                        j = (y + dy) * w + x + dx
                        if not is_bg[j]:
                            rim2[j] = 1
    rim = rim2

    for y in range(h):
        for x in range(w):
            i = y * w + x
            c = px[x, y]
            if is_bg[i]:
                op[x, y] = (c[0], c[1], c[2], 0)
                continue
            if not rim[i]:
                op[x, y] = (c[0], c[1], c[2], 255)
                continue
            # find a solid interior colour nearby (furthest from background within radius)
            best, bd = None, -1
            for dy in range(-R, R + 1):
                for dx in range(-R, R + 1):
                    xx, yy = x + dx, y + dy
                    if 0 <= xx < w and 0 <= yy < h:
                        j = yy * w + xx
                        if not is_bg[j] and not rim[j]:
                            cc = px[xx, yy]
                            d = sum((cc[k] - bg[k]) ** 2 for k in range(3))
                            if d > bd:
                                bd, best = d, cc
            if best is None or bd < 3 * 4 * 4:
                # near-white foreground (the airship's white seams): keep opaque
                a = 1.0 if diff(c) >= 2 or best is not None else 0.6
                f = best or c
            else:
                f = best
                num = sum((c[k] - bg[k]) * (f[k] - bg[k]) for k in range(3))
                a = max(0.0, min(1.0, num / bd))
            op[x, y] = (f[0], f[1], f[2], int(round(a * 255)))
    return out


def crop_to_content(im: Image.Image, pad_frac=0.04) -> Image.Image:
    bbox = im.getchannel("A").getbbox()
    im = im.crop(bbox)
    w, h = im.size
    pad = int(max(w, h) * pad_frac)
    out = Image.new("RGBA", (w + 2 * pad, h + 2 * pad), (0, 0, 0, 0))
    out.paste(im, (pad, pad))
    return out


def square(im: Image.Image, size: int, fill=0.92) -> Image.Image:
    w, h = im.size
    s = size * fill / max(w, h)
    nw, nh = max(1, round(w * s)), max(1, round(h * s))
    small = im.resize((nw, nh), Image.LANCZOS)
    if size <= 32:
        small = small.filter(ImageFilter.UnsharpMask(radius=0.6, percent=60, threshold=0))
    out = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    out.paste(small, ((size - nw) // 2, (size - nh) // 2), small)
    return out


def font(size, bold=False):
    for name in (["segoeuib.ttf", "seguisb.ttf"] if bold else ["segoeuil.ttf", "segoeui.ttf"]):
        p = Path("C:/Windows/Fonts") / name
        if p.exists():
            return ImageFont.truetype(str(p), size)
    return ImageFont.load_default()


def gradient_bg(w, h, top, bottom):
    bg = Image.new("RGB", (w, h))
    d = ImageDraw.Draw(bg)
    for y in range(h):
        t = y / max(1, h - 1)
        c = tuple(int(top[k] + (bottom[k] - top[k]) * t) for k in range(3))
        d.line([(0, y), (w, y)], fill=c)
    return bg


def wizard_images(master: Image.Image):
    INSTALLER.mkdir(parents=True, exist_ok=True)
    # Large: 164x314 at 100% (Inno uses up to 410x797 for high DPI) — render at 2.5x.
    for scale, suffix in ((1, ""), (2.5, "@2.5x")):
        W, H = int(164 * scale), int(314 * scale)
        bg = gradient_bg(W, H, (0x1b, 0x1f, 0x27), (0x10, 0x12, 0x17)).convert("RGBA")
        # soft colour glow behind the airship
        glow = Image.new("RGBA", (W, H), (0, 0, 0, 0))
        gd = ImageDraw.Draw(glow)
        cx, cy = W // 2, int(H * 0.40)
        r = int(W * 0.55)
        gd.ellipse([cx - r, cy - r // 2, cx + r, cy + r // 2], fill=(0x2a, 0x7d, 0xe1, 70))
        glow = glow.filter(ImageFilter.GaussianBlur(int(18 * scale)))
        bg = Image.alpha_composite(bg, glow)
        ship = master.copy()
        sw = int(W * 0.86)
        ship = ship.resize((sw, int(ship.height * sw / ship.width)), Image.LANCZOS)
        bg.alpha_composite(ship, ((W - sw) // 2, cy - ship.height // 2))
        d = ImageDraw.Draw(bg)
        f1 = font(int(26 * scale), bold=True)
        f2 = font(int(10 * scale))
        t = "Vector"
        tw = d.textlength(t, font=f1)
        d.text(((W - tw) / 2, H * 0.62), t, font=f1, fill=(0xf2, 0xf2, 0xf2))
        t2 = "Vector graphics editor"
        tw2 = d.textlength(t2, font=f2)
        d.text(((W - tw2) / 2, H * 0.62 + 34 * scale), t2, font=f2, fill=(0x9a, 0xa3, 0xb2))
        # stripe accent from the logo colours
        cols = [(0x22, 0x96, 0xf3), (0x02, 0x93, 0x44), (0x3d, 0xcf, 0x50), (0xc8, 0xdc, 0x10), (0xff, 0x7a, 0x00), (0xad, 0x1d, 0x56)]
        bw = W / len(cols)
        for i, c in enumerate(cols):
            d.rectangle([i * bw, H - 4 * scale, (i + 1) * bw, H], fill=c)
        bg.convert("RGB").save(INSTALLER / f"wizard-large{suffix}.bmp")
        # Small: 55x55
        S = int(55 * scale)
        small = Image.new("RGBA", (S, S), (255, 255, 255, 255))
        small.alpha_composite(square(master, S, 0.94))
        small.convert("RGB").save(INSTALLER / f"wizard-small{suffix}.bmp")


def main():
    src = Path(sys.argv[1])
    ASSETS.mkdir(parents=True, exist_ok=True)
    im = Image.open(src)
    clean = remove_background(im)
    master = crop_to_content(clean, 0.02)
    master.save(ASSETS / "airship.png", optimize=True)
    sizes = [16, 20, 24, 32, 40, 48, 64, 96, 128, 256]
    icons = [square(master, s) for s in sizes]
    icons[-1].save(ASSETS / "icon-256.png", optimize=True)
    icons[-1].save(ASSETS / "vector.ico", sizes=[(s, s) for s in sizes], append_images=icons[:-1])
    # explicit: Pillow builds the ICO from the largest image + sizes; re-save from per-size renders
    from io import BytesIO
    import struct

    entries = []
    blobs = []
    for s, img in zip(sizes, icons):
        b = BytesIO()
        img.save(b, "PNG")
        blobs.append(b.getvalue())
        entries.append(s)
    header = struct.pack("<HHH", 0, 1, len(sizes))
    offset = 6 + 16 * len(sizes)
    dir_bytes = b""
    for s, blob in zip(entries, blobs):
        dir_bytes += struct.pack("<BBBBHHII", s % 256, s % 256, 0, 0, 1, 32, len(blob), offset)
        offset += len(blob)
    (ASSETS / "vector.ico").write_bytes(header + dir_bytes + b"".join(blobs))
    # splash master (wider render for crisp display at high DPI)
    w = 900
    master.resize((w, int(master.height * w / master.width)), Image.LANCZOS).save(ASSETS / "splash-airship.png", optimize=True)
    wizard_images(master)
    print("ok", master.size)


if __name__ == "__main__":
    main()
