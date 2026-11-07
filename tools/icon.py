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

CAT_INK = (50, 45, 44)
CAT_BODY = (64, 58, 56)
CAT_CREAM = (246, 236, 219)
CAT_EAR = (226, 158, 154)
CAT_EYE = (236, 186, 80)
CAT_PUPIL = (28, 25, 24)
CAT_NOSE = (226, 132, 134)
CAT_WHISKER = (176, 166, 158)
TOP = (244, 178, 92)     # warm amber
BOTTOM = (204, 98, 64)   # terracotta


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


def draw_cat(d, cx, cy, k):
    def at(x, y):
        return (cx + x * k, cy + y * k)

    ellipse(d, at(0, 0.47), 0.27 * k, 0.17 * k, CAT_BODY)
    thick(d, cubic([at(0.14, 0.40), at(0.44, 0.42), at(0.58, 0.18), at(0.47, -0.10)]), 0.075 * k, CAT_BODY)

    for s in (-1, 1):
        outer = [at(0.285 * s, -0.04), at(0.255 * s, -0.355), at(0.055 * s, -0.175)]
        d.polygon(rounded_poly(outer, 0.18), fill=CAT_INK)
        inner = [at(0.235 * s, -0.09), at(0.225 * s, -0.285), at(0.105 * s, -0.17)]
        d.polygon(rounded_poly(inner, 0.22), fill=CAT_EAR)

    d.polygon(head_shape(at(0, 0.035), 0.305 * k, 0.255 * k), fill=CAT_INK)

    for s in (-1, 1):
        ellipse(d, at(0.068 * s, 0.135), 0.098 * k, 0.074 * k, CAT_CREAM)
    ellipse(d, at(0, 0.19), 0.07 * k, 0.05 * k, CAT_CREAM)

    for s in (-1, 1):
        e = at(0.125 * s, -0.012)
        ellipse(d, e, 0.07 * k, 0.06 * k, CAT_EYE)
        ellipse(d, e, 0.017 * k, 0.05 * k, CAT_PUPIL)
        ellipse(d, (e[0] + 0.022 * k, e[1] - 0.022 * k), 0.015 * k, 0.015 * k, (255, 255, 255))

    d.polygon(rounded_poly([at(-0.034, 0.078), at(0.034, 0.078), at(0, 0.113)], 0.3), fill=CAT_NOSE)

    mw = max(2, int(0.014 * k))
    d.line([at(0, 0.11), at(0, 0.135)], fill=CAT_INK, width=mw)
    for s in (-1, 1):
        d.line(quad(at(0, 0.135), at(0.022 * s, 0.172), at(0.062 * s, 0.142)), fill=CAT_INK, width=mw, joint="curve")

    ww = max(2, int(0.0105 * k))
    for s in (-1, 1):
        for i, dy in enumerate((-0.034, 0.0, 0.034)):
            tilt = (i - 1) * 0.045
            pts = quad(at(0.14 * s, 0.128 + dy), at(0.31 * s, 0.112 + dy + tilt * 0.5),
                       at(0.45 * s, 0.096 + dy * 1.4 + tilt))
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
