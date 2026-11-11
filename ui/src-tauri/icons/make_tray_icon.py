"""Render hard-sync's menu-bar template icon (black + alpha) from logo.svg's geometry.
No dependencies: SVG arcs are converted to polylines and each pixel's alpha is its
anti-aliased distance to the strokes."""
import math, struct, sys, zlib

STROKE = 40          # a bit lighter than the logo's 48: reads better at menu-bar size
# Paths from ui/logo.svg (arcs: start, radius, large-arc, sweep, end)
ARCS = [((126, 331), 150, 1, 1, (352, 141)), ((386, 181), 150, 1, 1, (160, 371))]
POLYS = [[(293, 115), (356, 140), (330, 203)], [(219, 397), (156, 372), (182, 309)]]

def arc_points(p1, r, large, sweep, p2, n=180):
    # SVG spec F.6.5: endpoint -> center parameterization (rotation 0)
    x1, y1 = p1; x2, y2 = p2
    dx, dy = (x1 - x2) / 2, (y1 - y2) / 2
    lam = (dx * dx + dy * dy) / (r * r)
    if lam > 1: r *= math.sqrt(lam)
    sign = -1 if large == sweep else 1
    co = sign * math.sqrt(max(0, (r*r*r*r - r*r*dy*dy - r*r*dx*dx) / (r*r*dy*dy + r*r*dx*dx)))
    cx_, cy_ = co * dy, -co * dx
    cx, cy = cx_ + (x1 + x2) / 2, cy_ + (y1 + y2) / 2
    a1 = math.atan2((dy - cy_) / r, (dx - cx_) / r)
    a2 = math.atan2((-dy - cy_) / r, (-dx - cx_) / r)
    d = a2 - a1
    if sweep and d < 0: d += 2 * math.pi
    if not sweep and d > 0: d -= 2 * math.pi
    return [(cx + r * math.cos(a1 + d * i / n), cy + r * math.sin(a1 + d * i / n)) for i in range(n + 1)]

segs = []
for a in ARCS:
    pts = arc_points(*a)
    segs += list(zip(pts, pts[1:]))
for poly in POLYS:
    segs += list(zip(poly, poly[1:]))

def dist(px, py, a, b):
    (ax, ay), (bx, by) = a, b
    vx, vy = bx - ax, by - ay
    t = max(0, min(1, ((px - ax) * vx + (py - ay) * vy) / (vx * vx + vy * vy or 1)))
    return math.hypot(px - ax - t * vx, py - ay - t * vy)

def render(size, pad_frac=0.06):
    # Fit the strokes' bounding box (incl. stroke width) into the icon
    xs = [p[0] for s in segs for p in s]; ys = [p[1] for s in segs for p in s]
    x0, x1 = min(xs) - STROKE / 2, max(xs) + STROKE / 2
    y0, y1 = min(ys) - STROKE / 2, max(ys) + STROKE / 2
    span = max(x1 - x0, y1 - y0)
    scale = size * (1 - 2 * pad_frac) / span
    cxl, cyl = (x0 + x1) / 2, (y0 + y1) / 2   # centre of the drawing, logo space
    half = STROKE / 2
    rows = []
    for y in range(size):
        row = bytearray()
        for x in range(size):
            # pixel center -> logo space
            lx = (x + 0.5 - size / 2) / scale + cxl
            ly = (y + 0.5 - size / 2) / scale + cyl
            d = min(dist(lx, ly, a, b) for a, b in segs)
            cov = max(0.0, min(1.0, (half - d) * scale + 0.5))  # 1px AA edge
            row += bytes((0, 0, 0, round(cov * 255)))
        rows.append(bytes(row))
    raw = b"".join(b"\x00" + r for r in rows)
    def chunk(t, d): return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))

out = sys.argv[1]
open(out, "wb").write(render(int(sys.argv[2]) if len(sys.argv) > 2 else 44))
print("wrote", out)
