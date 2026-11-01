"""Vectorises a white-on-black logo PNG into bezier loops (run with: blender -b -P trace_logo.py).
Writes assets/audiora_trace.json: parts = [{outer: loop, holes: [loop]}], loop = {start, segs}."""
import bpy, json, os, sys

HERE = r"C:\Users\rouja\Documents\GitHub\Audiora\intro-video"
sys.path.insert(0, os.path.join(HERE, "_pylib"))
import numpy as np
import potrace

img = bpy.data.images.load(os.path.join(HERE, "assets", "audiora-logo.png"))
w, h = img.size
px = np.array(img.pixels[:], dtype=np.float32).reshape(h, w, 4)      # rows bottom-up
mask = px[:, :, :3].mean(axis=2) > 0.5
print("size", w, h, "white px", int(mask.sum()))

bm = potrace.Bitmap(~mask)
plist = bm.trace(turdsize=12, alphamax=1.0, opticurve=True, opttolerance=0.25)


def loop(c):
    segs = []
    for s in c.segments:
        if s.is_corner:
            segs.append(["c", s.c.x, s.c.y, s.end_point.x, s.end_point.y])
        else:
            segs.append(["b", s.c1.x, s.c1.y, s.c2.x, s.c2.y, s.end_point.x, s.end_point.y])
    return {"start": [c.start_point.x, c.start_point.y], "segs": segs}


def poly(lp):
    pts = [tuple(lp["start"])] + [(g[-2], g[-1]) for g in lp["segs"]]
    return pts


def area(pts):
    return 0.5 * sum(pts[i][0] * pts[(i + 1) % len(pts)][1] - pts[(i + 1) % len(pts)][0] * pts[i][1] for i in range(len(pts)))


def inside(pt, pts):
    x, y, c = pt[0], pt[1], False
    for i in range(len(pts)):
        x1, y1 = pts[i]
        x2, y2 = pts[(i + 1) % len(pts)]
        if (y1 > y) != (y2 > y) and x < (x2 - x1) * (y - y1) / (y2 - y1) + x1:
            c = not c
    return c


loops = [loop(c) for c in plist]
polys = [poly(lp) for lp in loops]
areas = [area(p) for p in polys]
big = max(range(len(loops)), key=lambda i: abs(areas[i]))
outer_sign = areas[big] > 0
outers = [i for i in range(len(loops)) if (areas[i] > 0) == outer_sign]
parts = {i: {"outer": loops[i], "holes": []} for i in outers}
for i in range(len(loops)):
    if i in parts:
        continue
    cands = [o for o in outers if inside(polys[i][0], polys[o])]
    if cands:
        parts[min(cands, key=lambda o: abs(areas[o]))]["holes"].append(loops[i])
parts = list(parts.values())
print("parts", len(parts), "holes", sum(len(p["holes"]) for p in parts))
json.dump({"w": w, "h": h, "parts": parts}, open(os.path.join(HERE, "assets", "audiora_trace.json"), "w"))
