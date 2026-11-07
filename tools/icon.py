#!/usr/bin/env python3
"""Render the Cats Grid desktop icon (256x256 PNG).

Same design language as the in-app cat (src/main.rs `draw_cat`): charcoal head,
pink inner ears, amber slit eyes, cream muzzle, w-mouth, whiskers, tail curling
out of frame, all on a warm amber→terracotta rounded square.

    python3 tools/icon.py [out.png]    # default: ~/.local/share/icons/cats-grid.png
"""
import math
import os
import sys

from PIL import Image, ImageDraw, ImageFilter

SIZE = 256
SS = 4  # supersampling factor
S = SIZE * SS

CAT_LINE = (74, 42, 24)
CAT_FUR = (204, 132, 70)
CAT_FUR_LIGHT = (226, 156, 92)
CAT_FUR_DARK = (178, 108, 56)
CAT_STRIPE = (98, 56, 30)
CAT_CREAM = (254, 247, 236)
CAT_CREAM_SHADE = (226, 212, 192)
CAT_EAR = (238, 150, 172)
CAT_EYE = (120, 200, 64)
CAT_EYE_EDGE = (52, 130, 40)
CAT_PUPIL = (24, 20, 18)
CAT_NOSE = (240, 130, 148)
CAT_WHISKER = (214, 204, 196)
TOP = (126, 190, 196)    # soft teal
BOTTOM = (62, 128, 140)  # deeper teal


def lerp(a, b, t):
    return tuple(int(round(x + (y - x) * t)) for x, y in zip(a, b))


def quad(a, b, c, n=24):
    out = []
    for i in range(n + 1):
        t = i / n
        u = 1 - t
        out.append((u * u * a[0] + 2 * u * t * b[0] + t * t * c[0],
                    u * u * a[1] + 2 * u * t * b[1] + t * t * c[1]))
    return out


def cubic(p, n=48):
    out = []
    for i in range(n + 1):
        t = i / n
        u = 1 - t
        k = (u ** 3, 3 * u * u * t, 3 * u * t * t, t ** 3)
        out.append((sum(k[j] * p[j][0] for j in range(4)),
                    sum(k[j] * p[j][1] for j in range(4))))
    return out


def rounded_poly(pts, frac):
    out = []
    n = len(pts)
    for i in range(n):
        p, prev, nxt = pts[i], pts[i - 1], pts[(i + 1) % n]
        a = (p[0] + (prev[0] - p[0]) * frac, p[1] + (prev[1] - p[1]) * frac)
        b = (p[0] + (nxt[0] - p[0]) * frac, p[1] + (nxt[1] - p[1]) * frac)
        out.extend(quad(a, p, b, 8))
    return out


def head_shape(c, rx, ry, steps=160):
    pts = []
    e = 2 / 2.35
    for i in range(steps):
        t = i / steps * 2 * math.pi
        s, co = math.sin(t), math.cos(t)
        x = math.copysign(abs(co) ** e, co)
        y = math.copysign(abs(s) ** e, s)
        cheek = 1 + 0.06 * max(0.0, s)
        pts.append((c[0] + rx * x * cheek, c[1] + ry * y))
    return pts


def ellipse(d, c, rx, ry, fill):
    d.ellipse([c[0] - rx, c[1] - ry, c[0] + rx, c[1] + ry], fill=fill)


def thick(d, pts, w, fill):
    d.line(pts, fill=fill, width=int(w), joint="curve")
    for p in (pts[0], pts[-1]):
        ellipse(d, p, w / 2, w / 2, fill)


def outlined(d, pts, fill, k, w=0.014):
    d.polygon(pts, fill=fill)
    d.line(pts + [pts[0]], fill=CAT_LINE, width=max(2, int(w * k)), joint="curve")


def draw_cat(d, cx, cy, k):
    def at(x, y):
        return (cx + x * k, cy + y * k)

    def flip(pts, s):
        return pts if s < 0 else [pts[0], pts[2], pts[1]]

    body = [at(-0.12, 0.2), at(0.12, 0.2), at(0.30, 0.47), at(-0.30, 0.47)]
    outlined(d, rounded_poly(body, 0.16), CAT_FUR_DARK, k)
    ellipse(d, at(0, 0.40), 0.085 * k, 0.12 * k, CAT_CREAM)

    for s in (-1, 1):
        outer = flip([at(0.285 * s, -0.02), at(0.27 * s, -0.37), at(0.05 * s, -0.16)], s)
        outlined(d, rounded_poly(outer, 0.16), CAT_FUR, k)
        inner = flip([at(0.235 * s, -0.07), at(0.235 * s, -0.29), at(0.1 * s, -0.16)], s)
        d.polygon(rounded_poly(inner, 0.2), fill=CAT_EAR)

    outlined(d, head_shape(at(0, 0.035), 0.315 * k, 0.26 * k), CAT_FUR, k)
    ellipse(d, at(0, -0.04), 0.2 * k, 0.12 * k, CAT_FUR_LIGHT)

    sw = max(2, int(0.02 * k))
    for dx, ln in ((-0.07, 0.07), (0.0, 0.1), (0.07, 0.07)):
        thick(d, [at(dx, -0.215), at(dx * 0.85, -0.215 + ln)], sw, CAT_STRIPE)
    for s in (-1, 1):
        for y, ln in ((0.0, 0.1), (0.055, 0.115), (0.11, 0.09)):
            thick(d, [at(0.315 * s, y), at((0.315 - ln) * s, y + 0.015)], sw * 0.85, CAT_STRIPE)

    d.polygon(rounded_poly([at(0, -0.17), at(0.052, 0.045), at(-0.052, 0.045)], 0.22), fill=CAT_CREAM)
    for s in (-1, 1):
        ellipse(d, at(0.075 * s, 0.14), 0.11 * k, 0.085 * k, CAT_CREAM)
    ellipse(d, at(0, 0.2), 0.075 * k, 0.05 * k, CAT_CREAM)

    for s in (-1, 1):
        e = at(0.125 * s, -0.01)
        ellipse(d, e, 0.092 * k, 0.082 * k, CAT_LINE)
        ellipse(d, e, 0.082 * k, 0.072 * k, CAT_EYE_EDGE)
        ellipse(d, (e[0], e[1] - 0.008 * k), 0.078 * k, 0.062 * k, CAT_EYE)
        ellipse(d, e, 0.018 * k, 0.058 * k, CAT_PUPIL)
        ellipse(d, (e[0] + 0.026 * k, e[1] - 0.026 * k), 0.016 * k, 0.016 * k, (255, 255, 255))
        d.line(quad(at(0.125 * s - 0.09, -0.08), at(0.125 * s, -0.145), at(0.125 * s + 0.09, -0.08)),
               fill=CAT_LINE, width=max(2, int(0.014 * k)), joint="curve")

    nose = rounded_poly([at(-0.036, 0.085), at(0.036, 0.085), at(0, 0.12)], 0.3)
    outlined(d, nose, CAT_NOSE, k, 0.008)
    mw = max(2, int(0.013 * k))
    d.line([at(0, 0.12), at(0, 0.14)], fill=CAT_LINE, width=mw)
    for s in (-1, 1):
        d.line(quad(at(0, 0.14), at(0.022 * s, 0.178), at(0.065 * s, 0.148)), fill=CAT_LINE, width=mw, joint="curve")

    ww = max(2, int(0.01 * k))
    for s in (-1, 1):
        for i, dy in enumerate((-0.03, 0.01, 0.05)):
            tilt = (i - 1) * 0.05
            pts = quad(at(0.16 * s, 0.14 + dy), at(0.32 * s, 0.13 + dy + tilt * 0.5),
                       at(0.46 * s, 0.115 + dy * 1.3 + tilt))
            d.line(pts, fill=CAT_WHISKER, width=ww, joint="curve")


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else os.path.expanduser("~/.local/share/icons/cats-grid.png")
    margin = int(S * 0.06)
    radius = int(S * 0.22)
    box = [margin, margin, S - margin, S - margin]

    # warm diagonal gradient
    grad = Image.new("RGBA", (S, S))
    gp = grad.load()
    for y in range(S):
        for x in range(S):
            gp[x, y] = lerp(TOP, BOTTOM, min(1.0, (0.35 * x + 0.65 * y) / S)) + (255,)
    # soft top-left sheen
    sheen = Image.new("L", (S, S), 0)
    ImageDraw.Draw(sheen).ellipse([-S * 0.3, -S * 0.45, S * 0.9, S * 0.45], fill=46)
    sheen = sheen.filter(ImageFilter.GaussianBlur(S * 0.06))
    grad = Image.composite(Image.new("RGBA", (S, S), (255, 248, 236, 255)), grad, sheen)

    mask = Image.new("L", (S, S), 0)
    ImageDraw.Draw(mask).rounded_rectangle(box, radius, fill=255)

    # drop shadow
    shadow = Image.new("L", (S, S), 0)
    ImageDraw.Draw(shadow).rounded_rectangle([box[0], box[1] + S * 0.02, box[2], box[3] + S * 0.02], radius, fill=90)
    shadow = shadow.filter(ImageFilter.GaussianBlur(S * 0.022))
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    img.paste(Image.new("RGBA", (S, S), (80, 40, 20, 255)), (0, 0), shadow)

    # cat layer, clipped to the rounded square
    tile = grad.copy()
    draw_cat(ImageDraw.Draw(tile), S / 2, S / 2 + S * 0.03, S * 0.88)
    img.paste(tile, (0, 0), mask)

    # hairline highlight on the rim
    rim = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    ImageDraw.Draw(rim).rounded_rectangle(box, radius, outline=(255, 255, 255, 70), width=SS * 2)
    img = Image.alpha_composite(img, rim)

    img = img.resize((SIZE, SIZE), Image.LANCZOS)
    os.makedirs(os.path.dirname(out) or ".", exist_ok=True)
    img.save(out)
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
