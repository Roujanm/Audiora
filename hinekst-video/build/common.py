"""Shared constants and the ONE light-line function used by every scene."""
import numpy as np
import cv2
from PIL import Image, ImageDraw, ImageFont, ImageFilter

FPS = 60
W16, H16 = 1920, 1080
W9, H9 = 1080, 1920

BG = (7, 7, 7)
INK = (14, 14, 24)          # faint cool corner
AMBER = (255, 180, 0)       # single accent
WHITE = (255, 255, 255)

FONT_BOLD = r"C:\Windows\Fonts\segoeuib.ttf"
FONT_REG = r"C:\Windows\Fonts\segoeui.ttf"
FONT_SEMI = r"C:\Windows\Fonts\seguisb.ttf"


def font(size, bold=True):
    return ImageFont.truetype(FONT_BOLD if bold else FONT_REG, size)


def stage(w, h, t=0.0):
    """Near-black stage with a warm amber glow that drifts and breathes, faint cool corner, grain."""
    yy, xx = np.mgrid[0:h, 0:w].astype(np.float32)
    cx = w * (0.5 + 0.06 * np.sin(t * 0.7))
    cy = h * (0.55 + 0.05 * np.cos(t * 0.5))
    r = np.hypot(xx - cx, yy - cy) / (0.6 * max(w, h))
    glow = np.exp(-r * r * 2.2) * (0.55 + 0.1 * np.sin(t * 1.3))
    img = np.empty((h, w, 3), np.float32)
    for i, c in enumerate(BG):
        img[..., i] = c
    warm = np.array([60, 34, 6], np.float32)
    img += glow[..., None] * warm
    corner = np.clip(1 - np.hypot(xx - w, yy) / (0.5 * max(w, h)), 0, 1)[..., None]
    img += corner * np.array(INK, np.float32) * 0.8
    rng = np.random.default_rng(int(t * 60) % 997)
    img += rng.normal(0, 1.6, (h, w, 1)).astype(np.float32)
    return np.clip(img, 0, 255).astype(np.uint8)


def draw_light_line(frame, pts, head=1.0, tail=0.0, width=6, color=AMBER):
    """The light line. pts: Nx2 polyline. Draws the part between tail..head (0..1 of length)
    with a hot white core, amber body and a soft glow; the head is brightest."""
    pts = np.asarray(pts, np.float32)
    seg = np.hypot(*np.diff(pts, axis=0).T)
    cum = np.concatenate([[0], np.cumsum(seg)])
    total = cum[-1]
    a, b = tail * total, head * total
    if b <= a:
        return frame
    ts = np.linspace(a, b, max(2, int((b - a) / 3)))
    xs = np.interp(ts, cum, pts[:, 0])
    ys = np.interp(ts, cum, pts[:, 1])
    glow = np.zeros(frame.shape, np.uint8)
    core = np.zeros(frame.shape, np.uint8)
    n = len(ts)
    for i in range(n - 1):
        k = (i + 1) / n                       # brighter toward the head
        p0, p1 = (int(xs[i]), int(ys[i])), (int(xs[i + 1]), int(ys[i + 1]))
        c = tuple(int(v * (0.35 + 0.65 * k)) for v in color[::-1])
        cv2.line(glow, p0, p1, c, max(1, int(round(width * 3))), cv2.LINE_AA)
        cv2.line(core, p0, p1, (255, 255, 255) if k > 0.9 else c, max(2, int(round(width))), cv2.LINE_AA)
    glow = cv2.GaussianBlur(glow, (0, 0), width * 2.2)
    out = frame.astype(np.int32) + glow[..., ::-1].astype(np.int32) + core[..., ::-1].astype(np.int32)
    return np.clip(out, 0, 255).astype(np.uint8)


def text_img(txt, size, color=WHITE, bold=True):
    f = font(size, bold)
    l, t, r, b = f.getbbox(txt)
    im = Image.new("RGBA", (r - l + 8, b - t + 8), (0, 0, 0, 0))
    ImageDraw.Draw(im).text((4 - l, 4 - t), txt, font=f, fill=color + (255,))
    return im
