"""Draws the fictional sponsor logo "Beispiel-Sponsor GmbH" as an RGBA PNG.

Pure Python (zlib, struct, math); no image library. Letters are strokes on a 6 x 10 grid,
rasterised with anti-aliasing by their distance to the stroke centre line.

Usage: python3 make_sponsor_png.py beispiel-sponsor.png
The output is byte for byte the beispiel-sponsor.png next to this script.

Made for freemapper-print on 9 November 2026; dedicated to the public domain under CC0 1.0.
"""

import math
import struct
import sys
import zlib

W, H = 640, 200


def ellipse(cx, cy, rx, ry, a0, a1, n=24):
    return [
        (cx + rx * math.cos(math.radians(a0 + (a1 - a0) * i / n)),
         cy + ry * math.sin(math.radians(a0 + (a1 - a0) * i / n)))
        for i in range(n + 1)
    ]


def bezier(p0, p1, p2, p3, n=16):
    out = []
    for i in range(n + 1):
        t = i / n
        u = 1 - t
        out.append(tuple(u**3 * a + 3 * u * u * t * b + 3 * u * t * t * c + t**3 * d
                         for a, b, c, d in zip(p0, p1, p2, p3)))
    return out


S = (bezier((5.6, 1.4), (4.6, -0.2), (0.4, -0.2), (0.4, 2.6))
     + bezier((0.4, 2.6), (0.4, 5.4), (5.6, 4.6), (5.6, 7.4))
     + bezier((5.6, 7.4), (5.6, 10.3), (1.2, 10.4), (0.2, 8.6)))

# Letter: (advance width, list of polylines).
GLYPHS = {
    "B": (6, [[(0, 10), (0, 0), (3.2, 0)] + ellipse(3.2, 2.5, 2.5, 2.5, -90, 90) + [(0, 5)],
              [(3.4, 5)] + ellipse(3.4, 7.5, 2.6, 2.5, -90, 90) + [(0, 10)]]),
    "E": (5, [[(5, 0), (0, 0), (0, 10), (5, 10)], [(0, 5), (4.2, 5)]]),
    "I": (0, [[(0, 0), (0, 10)]]),
    "S": (6, [S]),
    "P": (6, [[(0, 10), (0, 0), (3.5, 0)] + ellipse(3.5, 2.6, 2.5, 2.6, -90, 90) + [(0, 5.2)]]),
    "L": (5, [[(0, 0), (0, 10), (5, 10)]]),
    "-": (4, [[(0, 5.5), (4, 5.5)]]),
    "O": (6, [ellipse(3, 5, 3, 5, 0, 360, 48)]),
    "N": (6, [[(0, 10), (0, 0), (6, 10), (6, 0)]]),
    "R": (6, [[(0, 10), (0, 0), (3.5, 0)] + ellipse(3.5, 2.6, 2.5, 2.6, -90, 90) + [(0, 5.2)],
              [(3, 5.2), (6, 10)]]),
    "G": (6, [ellipse(3, 5, 3, 5, -50, -360, 40) + [(3.4, 5)]]),
    "M": (6, [[(0, 10), (0, 0), (3, 6), (6, 0), (6, 10)]]),
    "H": (6, [[(0, 0), (0, 10)], [(6, 0), (6, 10)], [(0, 5), (6, 5)]]),
}
GAP = 2.5


def text_segments(text, x0, y0, scale):
    segs = []
    x = x0
    for ch in text:
        width, lines = GLYPHS[ch]
        for line in lines:
            pts = [(x + px * scale, y0 + py * scale) for px, py in line]
            segs.extend(zip(pts, pts[1:]))
        x += (width + GAP) * scale
    return segs


def stroke_coverage(segs, half):
    cov = [0.0] * (W * H)
    for (ax, ay), (bx, by) in segs:
        x0 = max(0, int(min(ax, bx) - half - 2))
        x1 = min(W - 1, int(max(ax, bx) + half + 2))
        y0 = max(0, int(min(ay, by) - half - 2))
        y1 = min(H - 1, int(max(ay, by) + half + 2))
        dx, dy = bx - ax, by - ay
        ll = dx * dx + dy * dy
        for y in range(y0, y1 + 1):
            py = y + 0.5
            for x in range(x0, x1 + 1):
                px = x + 0.5
                t = 0.0 if ll == 0 else max(0.0, min(1.0, ((px - ax) * dx + (py - ay) * dy) / ll))
                d = math.hypot(px - (ax + t * dx), py - (ay + t * dy))
                c = max(0.0, min(1.0, half + 0.5 - d))
                i = y * W + x
                if c > cov[i]:
                    cov[i] = c
    return cov


def rounded_rect_coverage(x0, y0, x1, y1, r):
    cov = [0.0] * (W * H)
    for y in range(H):
        py = y + 0.5
        for x in range(W):
            px = x + 0.5
            qx = max(x0 + r - px, 0, px - (x1 - r))
            qy = max(y0 + r - py, 0, py - (y1 - r))
            d = math.hypot(qx, qy) - r
            if qx == 0 and qy == 0:
                d = max(x0 - px, px - x1, y0 - py, py - y1)
            cov[y * W + x] = max(0.0, min(1.0, 0.5 - d))
    return cov


def circle_coverage(cx, cy, r):
    return [max(0.0, min(1.0, 0.5 - (math.hypot(x + 0.5 - cx, y + 0.5 - cy) - r)))
            for y in range(H) for x in range(W)]


def over(dst, colour, cov):
    for i, a in enumerate(cov):
        if a <= 0:
            continue
        r, g, b, da = dst[i]
        oa = a + da * (1 - a)
        mix = [(c * a + d * da * (1 - a)) / oa for c, d in zip(colour, (r, g, b))]
        dst[i] = (*mix, oa)


def main(out):
    pixels = [(0.0, 0.0, 0.0, 0.0)] * (W * H)
    over(pixels, (11, 110, 79), rounded_rect_coverage(4, 4, 636, 196, 40))
    over(pixels, (255, 204, 0), circle_coverage(100, 100, 62))
    over(pixels, (11, 110, 79), stroke_coverage(text_segments("B", 84, 70, 6.0), 4.5))
    over(pixels, (255, 255, 255), stroke_coverage(text_segments("BEISPIEL-SPONSOR", 190, 58, 3.6), 2.4))
    over(pixels, (255, 255, 255), stroke_coverage(text_segments("GMBH", 190, 112, 3.0), 2.1))

    raw = bytearray()
    for y in range(H):
        raw.append(0)
        for r, g, b, a in pixels[y * W:(y + 1) * W]:
            raw.extend((round(r), round(g), round(b), round(a * 255)))

    def chunk(kind, data):
        return (struct.pack(">I", len(data)) + kind + data
                + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF))

    png = (b"\x89PNG\r\n\x1a\n"
           + chunk(b"IHDR", struct.pack(">IIBBBBB", W, H, 8, 6, 0, 0, 0))
           + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
           + chunk(b"IEND", b""))
    with open(out, "wb") as f:
        f.write(png)


if __name__ == "__main__":
    main(sys.argv[1])
