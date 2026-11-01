"""HiNekst "Hi'dan Next'e" - 16:9 renderer. Every scene draws the light line through
common.draw_light_line.  Usage: render.py --preview | --final | --still T [T ...]"""
import sys, math, subprocess, argparse
from pathlib import Path
import numpy as np
import cv2
from PIL import Image, ImageDraw
import imageio_ffmpeg

ROOT = Path(r"C:\Users\rouja\Documents\GitHub\Audiora\hinekst-video")
sys.path.insert(0, str(ROOT / "build"))
from common import AMBER, WHITE, font, draw_light_line, text_img, FONT_REG

CAP = ROOT / "assets" / "captures"
DUR = 21.2
LAND = 0.17          # the slam
TAP = 6.03
HERO = 16.80

# ---------------------------------------------------------------- helpers
def clamp(x, a=0.0, b=1.0): return max(a, min(b, x))
def prog(t, a, b): return clamp((t - a) / (b - a))
def eio(x): return x * x * (3 - 2 * x)
def eo3(x): return 1 - (1 - x) ** 3
def eo5(x): return 1 - (1 - x) ** 5
def eback(x):
    c1 = 1.2; c3 = c1 + 1
    return 1 + c3 * (x - 1) ** 3 + c1 * (x - 1) ** 2
def lerp(a, b, k): return a + (b - a) * k

def load(p):
    return cv2.cvtColor(cv2.imread(str(p)), cv2.COLOR_BGR2RGB)

_rr = {}
def rr_alpha(h, w, r):
    key = (h, w, r)
    if key not in _rr:
        m = Image.new("L", (w * 2, h * 2), 0)
        ImageDraw.Draw(m).rounded_rectangle([0, 0, w * 2 - 1, h * 2 - 1], radius=r * 2, fill=255)
        _rr[key] = np.asarray(m.resize((w, h), Image.LANCZOS), np.float32) / 255.0
    return _rr[key]

_sh = {}
def shrunk(arr, f):
    if f == 1: return arr
    key = (id(arr), f)
    if key not in _sh:
        _sh[key] = cv2.resize(arr, (arr.shape[1] // f, arr.shape[0] // f), interpolation=cv2.INTER_AREA)
    return _sh[key]

def blit(cv, arr, alpha, k, a_src, a_dst, ang=0.0, op=1.0, shadow=0.0):
    """Affine-place `arr` (+float alpha) so src point a_src lands on a_dst; returns corner quad."""
    H, W = cv.shape[:2]
    lvl = 1
    while k * lvl < 0.5 and lvl < 16: lvl *= 2
    if lvl > 1:
        arr = shrunk(arr, lvl); alpha = shrunk(alpha, lvl)
        k = k * lvl; a_src = (a_src[0] / lvl, a_src[1] / lvl)
    h, w = arr.shape[:2]
    c, s_ = math.cos(math.radians(ang)), math.sin(math.radians(ang))
    A = np.array([[c * k, -s_ * k], [s_ * k, c * k]], np.float32)
    t = np.array(a_dst, np.float32) - A @ np.array(a_src, np.float32)
    corners = (A @ np.array([[0, 0], [w, 0], [w, h], [0, h]], np.float32).T).T + t
    if op <= 0: return corners
    pad = int(70 * shadow) + 2 if shadow else 2
    x0 = max(0, int(corners[:, 0].min()) - pad); x1 = min(W, int(corners[:, 0].max()) + pad + 1)
    y0 = max(0, int(corners[:, 1].min()) - pad); y1 = min(H, int(corners[:, 1].max()) + pad + 1)
    if x1 <= x0 or y1 <= y0: return corners
    M = np.concatenate([A, (t - np.array([x0, y0], np.float32))[:, None]], axis=1)
    size = (x1 - x0, y1 - y0)
    warp = cv2.warpAffine(arr, M, size, flags=cv2.INTER_LINEAR, borderMode=cv2.BORDER_CONSTANT)
    a = cv2.warpAffine(alpha, M, size, flags=cv2.INTER_LINEAR, borderMode=cv2.BORDER_CONSTANT)
    reg = cv[y0:y1, x0:x1].astype(np.float32)
    if shadow:
        sh = cv2.GaussianBlur(a, (0, 0), 22 * shadow)
        sh = np.roll(sh, int(16 * shadow), axis=0)
        reg *= (1 - 0.6 * sh * op)[..., None]
    aa = (a * op)[..., None]
    reg = reg * (1 - aa) + warp.astype(np.float32) * aa
    cv[y0:y1, x0:x1] = np.clip(reg, 0, 255).astype(np.uint8)
    return corners

_tx = {}
def T(txt, size, color=WHITE, bold=True):
    key = (txt, size, color, bold)
    if key not in _tx:
        f = font(size, bold)
        l, t, r, b = f.getbbox(txt)
        im = Image.new("RGBA", (r - l + 12, b - t + 12), (0, 0, 0, 0))
        ImageDraw.Draw(im).text((6 - l, 6 - t), txt, font=f, fill=tuple(color) + (255,))
        _tx[key] = np.asarray(im)
    return _tx[key]

def put(cv, rgba, cx, cy, op=1.0):
    H, W = cv.shape[:2]
    h, w = rgba.shape[:2]
    x0, y0 = int(cx - w / 2), int(cy - h / 2)
    sx0, sy0 = max(0, -x0), max(0, -y0)
    x0c, y0c = max(0, x0), max(0, y0)
    x1c, y1c = min(W, x0 + w), min(H, y0 + h)
    if x1c <= x0c or y1c <= y0c or op <= 0: return
    sub = rgba[sy0:sy0 + (y1c - y0c), sx0:sx0 + (x1c - x0c)].astype(np.float32)
    a = sub[..., 3:4] / 255.0 * op
    reg = cv[y0c:y1c, x0c:x1c].astype(np.float32)
    cv[y0c:y1c, x0c:x1c] = np.clip(reg * (1 - a) + sub[..., :3] * a, 0, 255).astype(np.uint8)

def resample(pts, n=140):
    pts = np.asarray(pts, np.float32)
    seg = np.hypot(*np.diff(pts, axis=0).T)
    cum = np.concatenate([[0], np.cumsum(seg)])
    u = np.linspace(0, cum[-1], n)
    return np.stack([np.interp(u, cum, pts[:, 0]), np.interp(u, cum, pts[:, 1])], 1)

def morph(a, b, k):
    return resample(a) * (1 - k) + resample(b) * k

def rect_path(x0, y0, x1, y1):
    return [(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)]

def add(cv, v):
    cv[:] = np.clip(cv.astype(np.int16) + int(v), 0, 255).astype(np.uint8)


# ---------------------------------------------------------------- renderer
class Film:
    def __init__(self, w, h):
        self.w, self.h = w, h
        self.s = w / 1920.0
        self.lw = max(2.0, 6 * self.s)
        s = self.s
        gx = np.linspace(0, 1, 48, dtype=np.float32)[None, :].repeat(27, 0)
        gy = np.linspace(0, 1, 27, dtype=np.float32)[:, None].repeat(48, 1)
        self.gx, self.gy = gx, gy
        # screenshots
        self.chat = load(CAP / "16x9/chat_squad_desktop-tr/full.png")
        self.meet = load(CAP / "16x9/meet_desktop-tr/full.png")
        self.guild = load(CAP / "16x9/guild_create_desktop-tr/full.png")
        self.store = load(CAP / "16x9/store_desktop-tr/full.png")
        self.desk_alpha = rr_alpha(1350, 2400, 26)
        self.mob0 = load(CAP / "9x16/chat_flow_mobile-tr/flow/00.png")
        mob3 = load(CAP / "9x16/chat_flow_mobile-tr/flow/03.png")
        self._build_phone(mob3)
        self._build_cards()
        # wordmark
        self.wm = T("HiNekst", int(230 * s))
        self.wm_cx, self.wm_cy = w / 2, 470 * s
        wmw = self.wm.shape[1] - 12
        x0, x1 = w / 2 - wmw / 2, w / 2 + wmw / 2
        u = np.linspace(0, 1, 120)
        self.L1 = np.stack([x0 + u * (x1 - x0), 650 * s + 30 * s * np.sin(np.pi * u)], 1)
        self.rng = np.random.default_rng(7)

    # ---- phone
    def _build_phone(self, mob3):
        H = self.h
        self.sh_ = int(1.4 * H)
        self.sw_ = int(self.sh_ * 1290 / 2796)
        self.pad = int(self.sh_ * 0.018)
        sw, sh, pad = self.sw_, self.sh_, self.pad
        BW, BH = sw + 2 * pad, sh + 2 * pad
        self.BW, self.BH = BW, BH
        self.k0 = 0.86 * H / BH
        body = Image.new("RGBA", (BW, BH), (0, 0, 0, 0))
        d = ImageDraw.Draw(body)
        r = int(BW * 0.14)
        d.rounded_rectangle([0, 0, BW - 1, BH - 1], r, fill=(84, 84, 90, 255))
        d.rounded_rectangle([3, 3, BW - 4, BH - 4], r - 3, fill=(22, 22, 25, 255))
        screen = Image.fromarray(cv2.resize(self.mob0, (sw, sh), interpolation=cv2.INTER_AREA))
        mask = Image.fromarray((rr_alpha(sh, sw, int(sw * 0.115)) * 255).astype(np.uint8))
        body.paste(screen, (pad, pad), mask)
        self.phone = np.asarray(body.convert("RGB"))
        self.phone_a = np.asarray(body.split()[3], np.float32) / 255.0
        self.mob_small = cv2.resize(self.mob0, (sw, sh), interpolation=cv2.INTER_AREA)
        # avatar crop from the mobile capture (Sen)
        hh, ww = mob3.shape[:2]
        av = mob3[int(0.505 * hh):int(0.5525 * hh), int(0.845 * ww):int(0.948 * ww)]
        self.avatar = cv2.resize(av, (int(0.1 * sw), int(0.1 * sw)), interpolation=cv2.INTER_AREA)
        # input field fill sampled from the capture
        self.ix0, self.ix1 = 99 / 430, 376 / 430
        self.iy0, self.iy1 = 821 / 932, 863 / 932
        py = int((self.iy0 + 0.1 * (self.iy1 - self.iy0)) * self.mob0.shape[0])
        px = int((self.ix0 + 0.3 * (self.ix1 - self.ix0)) * self.mob0.shape[1])
        self.field_fill = tuple(int(v) for v in np.median(self.mob0[py - 3:py + 3, px - 3:px + 3].reshape(-1, 3), 0))
        self.btn = (0.923, 0.903)

    def phone_img(self, t):
        if not (2.45 <= t <= 7.4):
            return self.phone
        sw, sh, pad = self.sw_, self.sh_, self.pad
        im = Image.fromarray(self.phone.copy())
        d = ImageDraw.Draw(im)
        fx0, fx1 = pad + self.ix0 * sw + 5, pad + self.ix1 * sw - 5
        fy0, fy1 = pad + self.iy0 * sh + 3, pad + self.iy1 * sh - 3
        d.rounded_rectangle([fx0, fy0, fx1, fy1], radius=int((fy1 - fy0) / 2), fill=self.field_fill)
        fs = int(0.0215 * sh)
        f = font(fs, False)
        msg = "Ben de varım, 22:00'de lobide!"
        sent = t >= TAP
        n = 0 if sent else int(prog(t, 2.55, 3.45) * len(msg) + 1e-6)
        if t >= 2.5 and not sent:
            txt = msg[:n]
            ty = (fy0 + fy1) / 2
            d.text((fx0 + 0.012 * sw, ty), txt, font=f, fill=(240, 240, 244), anchor="lm")
            if int(t * 2.5) % 2 == 0 or n < len(msg):
                cx = fx0 + 0.012 * sw + d.textlength(txt, font=f) + 3
                d.rectangle([cx, ty - fs * 0.6, cx + 3, ty + fs * 0.6], fill=AMBER)
        elif sent:
            d.text((fx0 + 0.012 * sw, (fy0 + fy1) / 2), "Mesajınızı yazın…", font=f, fill=(112, 112, 124), anchor="lm")
        # send button (amber) - appears as the line arrives, fills at the tap
        bx, by = pad + self.btn[0] * sw, pad + self.btn[1] * sh
        fillk = eo3(prog(t, 5.85, 5.97)) if not sent else 1.0
        if fillk > 0:
            rad = 0.043 * sw * (0.15 + 0.85 * fillk)
            if sent:
                rad *= 1 + 0.18 * math.exp(-(t - TAP) * 14) * math.sin((t - TAP) * 30 + 1.2)
            d.ellipse([bx - rad, by - rad, bx + rad, by + rad], fill=AMBER)
            ah = rad * 0.42
            d.line([bx, by + ah, bx, by - ah], fill=(20, 14, 0), width=max(2, int(rad * 0.16)))
            d.line([bx - ah * 0.8, by - ah * 0.1, bx, by - ah], fill=(20, 14, 0), width=max(2, int(rad * 0.16)))
            d.line([bx + ah * 0.8, by - ah * 0.1, bx, by - ah], fill=(20, 14, 0), width=max(2, int(rad * 0.16)))
        if sent:
            q = eo3(prog(t, TAP + 0.03, TAP + 0.22))
            if q > 0:
                bx0, bx1 = pad + 0.186 * sw, pad + 0.823 * sw
                by0, by1 = pad + 0.529 * sh, pad + 0.5705 * sh
                yo = (1 - q) * 0.02 * sh
                lay = Image.new("RGBA", im.size, (0, 0, 0, 0))
                dl = ImageDraw.Draw(lay)
                dl.rounded_rectangle([bx0, by0 + yo, bx1, by1 + yo], radius=int((by1 - by0) / 2), fill=(229, 229, 233, int(255 * q)))
                dl.text((bx0 + 0.03 * sw, (by0 + by1) / 2 + yo), msg, font=f, fill=(18, 18, 22, int(255 * q)), anchor="lm")
                dl.text((bx1 - 0.03 * sw, (by0 + by1) / 2 + yo), "21:51", font=font(int(0.014 * sh), False), fill=(90, 90, 100, int(255 * q)), anchor="rm")
                dl.text((bx1 + 0.0 * sw, pad + 0.515 * sh + yo), "Sen", font=font(int(0.0185 * sh)), fill=(255, 255, 255, int(255 * q)), anchor="rm")
                av = self.avatar
                am = Image.fromarray((rr_alpha(av.shape[0], av.shape[1], av.shape[0] // 2) * 255 * q).astype(np.uint8))
                lay.paste(Image.fromarray(av), (int(pad + 0.845 * sw), int(pad + 0.505 * sh + yo)), am)
                im = Image.alpha_composite(im.convert("RGBA"), lay).convert("RGB")
        return np.asarray(im)

    def phone_xf(self, t):
        """k, a_src (base px), a_dst (canvas) for the phone; focal = screen (0.5, 0.9)."""
        w, h = self.w, self.h
        sw, sh, pad = self.sw_, self.sh_, self.pad
        a_src = (pad + 0.5 * sw, pad + 0.9 * sh)
        centre = (self.BW / 2, self.BH / 2)
        a_dst0 = (0.5 * w, 0.52 * h + self.k0 * (a_src[1] - centre[1]))
        rise = 1 - eback(prog(t, 1.9, 2.4)) if t < 2.4 else 0.0
        q = eio(prog(t, 4.8, 5.6))
        z = 1 + 0.95 * q
        dst = (lerp(a_dst0[0], 0.5 * w, q), lerp(a_dst0[1], 0.60 * h, q) + rise * 1.1 * h)
        d = eio(prog(t, 6.3, 7.2))
        z *= math.exp(math.log(4.5) * d)
        return self.k0 * z, a_src, dst

    def scr(self, xf, fx, fy):
        k, a_src, dst = xf
        bx, by = self.pad + fx * self.sw_, self.pad + fy * self.sh_
        return (dst[0] + k * (bx - a_src[0]), dst[1] + k * (by - a_src[1]))

    # ---- message cards peeled from the phone screen
    def _build_cards(self):
        hh, ww = self.mob0.shape[:2]
        self.cards = []
        for (fx0, fx1, fy0, fy1) in [(0.054, 0.73, 0.19, 0.258), (0.054, 0.62, 0.40, 0.468), (0.217, 0.953, 0.297, 0.365)]:
            c = self.mob0[int(fy0 * hh):int(fy1 * hh), int(fx0 * ww):int(fx1 * ww)].copy()
            a = rr_alpha(c.shape[0], c.shape[1], c.shape[0] // 4)
            self.cards.append((c, a))

    # ---- stage
    def stage(self, t):
        w, h = self.w, self.h
        cx = 0.5 + 0.06 * math.sin(t * 0.7); cy = 0.55 + 0.05 * math.cos(t * 0.5)
        r = np.hypot(self.gx - cx, (self.gy - cy) * 0.56) / 0.6
        glow = np.exp(-r * r * 2.2) * (0.55 + 0.1 * math.sin(t * 1.3))
        corner = np.clip(1 - np.hypot(self.gx - 1, self.gy * 0.56) / 0.5, 0, 1)
        img = np.empty((27, 48, 3), np.float32)
        img[...] = (7, 7, 7)
        img += glow[..., None] * np.array([60, 34, 6], np.float32)
        img += corner[..., None] * np.array([14, 14, 24], np.float32) * 0.8
        img = cv2.resize(img, (w, h), interpolation=cv2.INTER_CUBIC)
        rng = np.random.default_rng(int(t * 60) % 997)
        img += rng.normal(0, 1.6, (h, w, 1)).astype(np.float32)
        return np.clip(img, 0, 255).astype(np.uint8)

    # ---- the light line
    def line(self, cv, pts, head=1.0, tail=0.0, wmul=1.0, arrow=False):
        if head - tail <= 0.002: return cv
        cv = draw_light_line(cv, pts, head, tail, self.lw * wmul)
        if arrow and head > 0.98:
            p = np.asarray(pts, np.float32)
            d = p[-1] - p[-4]; ang = math.atan2(d[1], d[0]); L = 40 * self.s
            for sgn in (-1, 1):
                a = ang + math.pi + sgn * 0.55
                e = (p[-1][0] + L * math.cos(a), p[-1][1] + L * math.sin(a))
                cv = draw_light_line(cv, [e, p[-1]], 1.0, 0.0, self.lw * 0.9)
        return cv

    def wordmark(self, cv, op=1.0, reveal=1.0, dy=0.0, blur=0.0, scale=1.0):
        wm = self.wm
        if scale != 1.0:
            wm = cv2.resize(wm, None, fx=scale, fy=scale, interpolation=cv2.INTER_AREA)
        if reveal < 1.0:
            wm = wm.copy()
            xs = np.arange(wm.shape[1], dtype=np.float32)
            edge = reveal * (wm.shape[1] + 80 * self.s) - 40 * self.s
            m = np.clip((edge - xs) / (40 * self.s) + 0.5, 0, 1)
            wm[..., 3] = (wm[..., 3] * m[None, :]).astype(np.uint8)
        if blur > 0.3:
            wm = cv2.GaussianBlur(wm, (0, 0), blur)
        put(cv, wm, self.wm_cx, self.wm_cy + dy, op)

    def desk(self, cv, img, z, a_src, a_dst, op=1.0, shadow=True):
        w = self.w
        k = z * 0.9 * w / 2400.0
        return blit(cv, img, self.desk_alpha, k, a_src, a_dst, op=op, shadow=self.s if shadow else 0)

    # ------------------------------------------------------------ frame
    def frame(self, t):
        w, h, s = self.w, self.h, self.s
        cv = self.stage(t)
        shake = (0.0, 0.0)

        # ============ S1: the slam (0 - 1.6)
        if t < 2.8:
            wm_op = 1 - eio(prog(t, 1.6, 2.4))
            wm_dy = -150 * s * eio(prog(t, 1.6, 2.5))
            rev = eio(prog(t, 0.23, 0.67))
            if t >= 0.23 and wm_op > 0:
                self.wordmark(cv, wm_op, rev, wm_dy)
            if t < LAND:
                p = t / LAND
                e = 0.10 + 0.90 * p ** 2.4
                V = np.array([w / 2, 330 * s])
                for j, (oy, ox) in enumerate([(0, 0), (-70, 160), (60, -190)]):
                    Vj = V + np.array([ox, oy]) * s * (1 - e) * 2
                    pts = Vj + (self.L1 - Vj) * e
                    cv = draw_light_line(cv, pts, 1.0, 0.0, self.lw * (0.9 + 2.2 * (1 - e)) * (1.0 if j == 0 else 0.6))
            elif t < 1.9:
                # settled smile + pulse that runs a -> z and reveals the wordmark
                k_morph = eio(prog(t, 1.9, 2.7))
                cv = self.line(cv, self.L1, 1.0, 0.0, 1.0, arrow=True)
                if 0.23 <= t <= 0.8:
                    pp = eio(prog(t, 0.23, 0.67))
                    cv = draw_light_line(cv, self.L1, pp, max(0, pp - 0.22), self.lw * 2.0)
                # shards
                if t < 0.55:
                    q = (t - LAND) / 0.38
                    C = np.array([w / 2, 650 * s + 30 * s])
                    rng = np.random.default_rng(3)
                    lay = np.zeros_like(cv)
                    for _ in range(34):
                        a = rng.uniform(0, 2 * math.pi) if rng.random() > 0.5 else rng.uniform(math.pi * 1.0, math.pi * 2.0)
                        v = rng.uniform(300, 900) * s
                        d0 = v * eo3(q) * 0.5; d1 = d0 + v * 0.18 * (1 - q)
                        p0 = C + np.array([math.cos(a), math.sin(a)]) * d0
                        p1 = C + np.array([math.cos(a), math.sin(a)]) * d1
                        col = tuple(int(c * (1 - q)) for c in (255, 200, 90))
                        cv2.line(lay, tuple(int(x) for x in p0), tuple(int(x) for x in p1), col, max(1, int(2.5 * s)), cv2.LINE_AA)
                    cv = np.clip(cv.astype(np.int16) + lay.astype(np.int16), 0, 255).astype(np.uint8)
            else:
                pass
            if LAND <= t < 0.45:
                dec = math.exp(-(t - LAND) * 11)
                shake = (self.rng.normal(0, 14 * s) * dec, self.rng.normal(0, 14 * s) * dec)
                add(cv, int(70 * math.exp(-(t - LAND) * 26)))
            # S1 -> S2 line hand-over
            if t >= 1.9 and t < 2.8:
                k_m = eio(prog(t, 1.9, 2.75))
                xf = self.phone_xf(2.75)
                P = [self.scr(xf, self.ix0, self.iy1 + 0.004), self.scr(xf, self.ix1, self.iy1 + 0.004)]
                m = morph(self.L1, P, k_m)
                m[:, 1] -= 170 * s * math.sin(math.pi * k_m)
                cv = self.line(cv, m, 1.0, 0.0, 1.0)

        # ============ S2 + S3: phone
        if 1.9 <= t < 7.3:
            xf = self.phone_xf(t)
            k, a_src, dst = xf
            pa = 1.0 - eio(prog(t, 6.85, 7.25))
            # cards fan (3.4 - 4.37)
            self._cards(cv, t, xf)
            # backlight bloom
            blit(cv, self.phone_img(t), self.phone_a, k, a_src, dst, op=pa, shadow=s)
            # focus line: input underline (2.75 - 4.3)
            P = [self.scr(xf, self.ix0, self.iy1 + 0.004), self.scr(xf, self.ix1, self.iy1 + 0.004)]
            if 2.75 <= t < 4.3 and t > 2.7:
                cv = self.line(cv, P, 1.0, 0.0, 0.8 + 0.2 * math.sin(t * 9))
            # trace card 1
            if 4.3 <= t < 5.3:
                loop = self._card_loop(xf, t)
                if loop is not None:
                    mk = eio(prog(t, 4.3, 4.55))
                    base = morph(P, loop, mk)
                    head = eio(prog(t, 4.3, 4.8)) if mk >= 1 else 1.0
                    tail = eio(prog(t, 4.85, 5.3))
                    cv = self.line(cv, base, 1.0 if mk < 1 else head, tail)
            # line -> send button (5.5 - 5.97)
            btn = self.scr(xf, *self.btn)
            P3 = [self.scr(xf, self.ix0, self.iy1 + 0.004), self.scr(xf, self.ix1 + 0.02, self.iy1 + 0.004), btn]
            if 5.5 <= t < TAP + 0.05:
                hd = eio(prog(t, 5.5, 5.97))
                cv = self.line(cv, P3, hd, 0.0, 1.0)
            # shoot out after the tap
            if TAP + 0.03 <= t < 7.1:
                ext = np.array([btn[0] + 1.5 * w, btn[1] - 1.1 * w])
                pts = P3 + [tuple(ext)]
                seg = np.hypot(*np.diff(np.asarray(pts), axis=0).T)
                a_len = seg[:2].sum(); tot = seg.sum()
                e = eo3(prog(t, TAP + 0.05, TAP + 0.5))
                head = (a_len + (tot - a_len) * e) / tot
                tail = head * 0.0 + (a_len / tot) * eio(prog(t, TAP + 0.1, TAP + 0.45))
                cv = self.line(cv, pts, head, tail, 1.3 - 0.4 * e)
                if t < TAP + 0.4:
                    ring = prog(t, TAP, TAP + 0.4)
                    rad = (0.04 + 0.16 * eo3(ring)) * self.sw_ * k
                    ov = np.zeros_like(cv)
                    cv2.circle(ov, (int(btn[0]), int(btn[1])), int(rad), (int(200 * (1 - ring)), int(150 * (1 - ring)), 0), max(1, int(3 * s)), cv2.LINE_AA)
                    cv = np.clip(cv.astype(np.int16) + ov, 0, 255).astype(np.uint8)
            if TAP <= t < TAP + 0.08:
                shake = (self.rng.normal(0, 6 * s), self.rng.normal(0, 6 * s))

        # ============ S4: desktop chat + meet (6.8 - 10.5)
        if 6.8 <= t < 10.6:
            # camera: arrive zoomed on the voice list, pull back to the full window
            q = eio(prog(t, 7.2, 8.9))
            z = lerp(2.7, 1.0, q)
            a_src = (lerp(260, 1200, q), lerp(470, 675, q))
            a_dst = (w / 2, h / 2)
            arr = 1 - eio(prog(t, 6.8, 7.3))
            op_chat = eio(prog(t, 6.8, 7.25)) * (1 - eio(prog(t, 9.0, 9.7)))
            zc = z * (1 + 0.45 * (1 - eio(prog(t, 6.8, 7.4))))
            k = zc * 0.9 * w / 2400.0
            def d2c(x, y): return (a_dst[0] + k * (x - a_src[0]), a_dst[1] + k * (y - a_src[1]))
            if op_chat > 0:
                self.desk(cv, self.chat, zc, a_src, a_dst, op_chat)
                av = [(186, 406), (186, 517), (186, 572), (186, 629)]
                pts = []
                for i, (ax, ay) in enumerate(av):
                    pts.append(d2c(ax, ay))
                pts = [tuple(p) for p in resample(pts, 60)]
                hd = eio(prog(t, 7.4, 8.4)); tl = eio(prog(t, 8.55, 9.1))
                cv = self.line(cv, pts, hd, tl, 1.0 if tl <= 0 else 1.0)
                # speaking pulses
                for i, (ax, ay) in enumerate(av):
                    if hd > i / 3.0 - 0.001 and t < 9.0:
                        c = d2c(ax, ay)
                        pr = (t * 1.6 + i * 0.3) % 1.0
                        ov = np.zeros_like(cv)
                        cv2.circle(ov, (int(c[0]), int(c[1])), int((26 + 24 * pr) * k * 2.4 / 2.4 * 1.0), (int(160 * (1 - pr)), int(110 * (1 - pr)), 0), max(1, int(2 * s)), cv2.LINE_AA)
                        cv = np.clip(cv.astype(np.int16) + ov, 0, 255).astype(np.uint8)
            # meet
            q2 = eio(prog(t, 8.9, 9.7))
            op_meet = q2 * (1 - eio(prog(t, 10.0, 10.55)))
            if op_meet > 0:
                zm = lerp(0.93, 1.0, q2) * (1 + 0.04 * eio(prog(t, 10.0, 10.55)))
                self.desk(cv, self.meet, zm, (1200, 675), (w / 2, h / 2), op_meet)
                k2 = zm * 0.9 * w / 2400.0
                def m2c(x, y): return (w / 2 + k2 * (x - 1200), h / 2 + k2 * (y - 675))
                P = [m2c(*p) for p in rect_path(494, 180, 926, 422)]
                hd = eio(prog(t, 9.3, 9.95))
                cv = self.line(cv, P, hd, eio(prog(t, 10.1, 10.5)), 1.0)

        # ============ S5: server wizard + store (10.2 - 13.4)
        if 10.2 <= t < 13.5:
            op_g = eio(prog(t, 10.2, 10.7)) * (1 - eio(prog(t, 12.45, 13.0)))
            q = eio(prog(t, 10.5, 11.2)) * (1 - eio(prog(t, 11.5, 12.2)) * 0.8)
            z = lerp(1.0, 1.7, q)
            a_src = (lerp(1200, 870, q), lerp(675, 440, q))
            zz = z * lerp(0.94, 1.0, eio(prog(t, 10.2, 10.8)))
            if op_g > 0:
                self.desk(cv, self.guild, zz, a_src, (w / 2, h / 2), op_g)
                k = zz * 0.9 * w / 2400.0
                def g2c(x, y): return (w / 2 + k * (x - a_src[0]), h / 2 + k * (y - a_src[1]))
                P1 = [g2c(*p) for p in rect_path(678, 406, 1054, 476)]
                P2 = [g2c(*p) for p in rect_path(1110, 262, 2214, 936)]
                cv = self.line(cv, P1, eio(prog(t, 10.7, 11.25)), eio(prog(t, 11.35, 11.7)))
                cv = self.line(cv, P2, eio(prog(t, 11.4, 12.2)), eio(prog(t, 12.2, 12.6)))
            op_s = eio(prog(t, 12.4, 13.0)) * (1 - eio(prog(t, 13.0, 13.45)))
            if op_s > 0:
                zs = lerp(0.95, 1.04, eio(prog(t, 12.4, 13.45)))
                self.desk(cv, self.store, zs, (1200, 675), (w / 2, h / 2), op_s)
            # "Kendi sunucun." lands on the beat (12.0)  + notification
            if 12.0 <= t < 13.2:
                qn = eback(prog(t, 12.0, 12.35)); fo = 1 - eio(prog(t, 12.95, 13.25))
                pw, ph = 620 * s, 96 * s
                cx, cy = w / 2, (50 + 82 * min(qn, 1.05)) * s
                lay = np.zeros((int(ph) + 40, int(pw) + 40, 4), np.uint8)
                pil = Image.fromarray(lay)
                d = ImageDraw.Draw(pil)
                d.rounded_rectangle([20, 20, 20 + pw, 20 + ph], radius=int(ph / 2), fill=(32, 32, 38, 245), outline=(70, 70, 80, 255), width=max(1, int(2 * s)))
                d.ellipse([20 + 24 * s, 20 + 24 * s, 20 + 72 * s, 20 + 72 * s], fill=AMBER)
                d.text((20 + 98 * s, 20 + 32 * s), "HiNekst", font=font(int(24 * s)), fill=(255, 255, 255, 255), anchor="lm")
                d.text((20 + 98 * s, 20 + 64 * s), "Sunucun hazır · Gece Kuşları", font=font(int(21 * s), False), fill=(200, 200, 210, 255), anchor="lm")
                put(cv, np.asarray(pil), cx, cy, fo)

        # ============ S6: "Merhaba'dan ... Sonrakine." (13.3 - 16.5)
        if 13.3 <= t < 16.6:
            fo = 1 - eio(prog(t, 16.0, 16.45))
            t1 = T("Merhaba'dan…", int(170 * s))
            t2 = T("Sonrakine.", int(170 * s), AMBER)
            self._filled(cv, t1, w / 2, 420 * s, prog(t, 13.5, 14.35), eio(prog(t, 14.3, 14.9)), fo)
            self._filled(cv, t2, w / 2, 620 * s, prog(t, 15.1, 15.75), 1.0, fo)
            x0 = w / 2 - (t1.shape[1] - 12) / 2; x1 = w / 2 + (t1.shape[1] - 12) / 2
            u = np.linspace(0, 1, 100)
            self.L6 = np.stack([x0 + u * (x1 - x0), 780 * s + 36 * s * np.sin(np.pi * u)], 1)
            if t < 16.35:
                cv = self.line(cv, self.L6, eio(prog(t, 15.2, 16.0)), 0.0, 1.0, arrow=True)

        # ============ S7/S8: hero (16.3 - 21.2)
        if t >= 16.3:
            if t < 16.8:
                k_m = eio(prog(t, 16.3, 16.8))
                m = morph(self.L6, self.L1, k_m)
                cv = self.line(cv, m, 1.0, 0.0, 0.6 + 0.6 * k_m)
            elif t < 19.3:
                dis = prog(t, 18.55, 19.2)
                rev = eio(prog(t, HERO, HERO + 0.45))
                self.wordmark(cv, 1 - eio(prog(t, 18.9, 19.3)), rev, 0, blur=(26 * s) * eio(dis))
                tail = eio(prog(t, 18.6, 19.15))
                cv = self.line(cv, self.L1, 1.0, tail, 1.0 - 0.2 * tail, arrow=(tail < 0.5))
                pp = ((t - 17.35) / 2.0) % 1.0
                if 17.35 <= t < 18.6:
                    cv = draw_light_line(cv, self.L1, pp, max(0, pp - 0.18), self.lw * 1.6)
                if t >= 18.9:
                    ov = np.zeros_like(cv)
                    e = self.L1[-1]
                    a = 1 - eio(prog(t, 19.1, 19.45))
                    cv2.circle(ov, (int(e[0]), int(e[1])), max(2, int(7 * s)), (int(255 * a), int(190 * a), int(60 * a)), -1, cv2.LINE_AA)
                    ov = cv2.GaussianBlur(ov, (0, 0), 6 * s) * 3 + ov
                    cv = np.clip(cv.astype(np.int16) + ov, 0, 255).astype(np.uint8)
                if HERO <= t < HERO + 0.35:
                    dec = math.exp(-(t - HERO) * 12)
                    add(cv, int(80 * dec))
                    shake = (self.rng.normal(0, 9 * s) * dec, self.rng.normal(0, 9 * s) * dec)
            if t >= 19.2:
                fo = eio(prog(t, 19.3, 19.8))
                sig = T("HiNekst", int(120 * s))
                put(cv, sig, w / 2, 500 * s, fo)
                u = np.linspace(0, 1, 60)
                x0 = w / 2 - (sig.shape[1] - 12) / 2; x1 = w / 2 + (sig.shape[1] - 12) / 2
                Ls = np.stack([x0 + u * (x1 - x0), 590 * s + 14 * s * np.sin(np.pi * u)], 1)
                cv = self.line(cv, Ls, eio(prog(t, 19.4, 20.0)), 0.0, 0.6, arrow=True)
                put(cv, T("BETA", int(26 * s), (255, 255, 255)), w / 2, 665 * s, fo * 0.85)
                put(cv, T("Merhaba'dan Sonrakine.", int(30 * s), (170, 170, 180), False), w / 2, 730 * s, fo * 0.8)

        # camera drift + shake
        dx = 6 * s * math.sin(t * 0.55) + shake[0]
        dy = 4 * s * math.cos(t * 0.43) + shake[1]
        if abs(dx) > 0.3 or abs(dy) > 0.3:
            M = np.float32([[1, 0, dx], [0, 1, dy]])
            cv = cv2.warpAffine(cv, M, (w, h), borderMode=cv2.BORDER_REPLICATE)
        # dive blur (fly through the screen)
        db = eio(prog(t, 6.55, 7.05)) * (1 - eio(prog(t, 7.05, 7.5)))
        if db > 0.02:
            cv = cv2.GaussianBlur(cv, (0, 0), 9 * s * db)
        return cv

    # ---- helpers used in frame()
    def _card_targets(self):
        w, h = self.w, self.h
        return [((0.19 * w, 0.30 * h), -6, 1.35), ((0.17 * w, 0.68 * h), 5, 1.25), ((0.82 * w, 0.50 * h), 7, 1.35)]

    def _card_geom(self, i, t, xf):
        k, a_src, dst = xf
        (tx, ty), ang, sc = self._card_targets()[i]
        c, a = self.cards[i]
        fx0, fx1, fy0, fy1 = [(0.054, 0.73, 0.19, 0.258), (0.054, 0.62, 0.40, 0.468), (0.217, 0.953, 0.297, 0.365)][i]
        o = self.scr(xf, (fx0 + fx1) / 2, (fy0 + fy1) / 2)
        q = eo5(prog(t, 3.40 + 0.12 * i, 4.37 - 0.12 * (2 - i)))
        kk = lerp(k * self.sw_ / c.shape[1] * (fx1 - fx0), sc * 0.34 * self.w / c.shape[1], q)
        pos = (lerp(o[0], tx, q), lerp(o[1], ty, q))
        return c, a, kk, pos, ang * q, q

    def _cards(self, cv, t, xf):
        if t < 3.4: return
        fade = 1 - eio(prog(t, 5.0, 5.5))
        for i in range(3):
            c, a, kk, pos, ang, q = self._card_geom(i, t, xf)
            blit(cv, c, a, kk, (c.shape[1] / 2, c.shape[0] / 2), pos, ang, op=fade * clamp(q * 4), shadow=self.s)

    def _card_loop(self, xf, t):
        c, a, kk, pos, ang, q = self._card_geom(0, 4.4, xf)
        co = blit(np.zeros((2, 2, 3), np.uint8), c, a, kk, (c.shape[1] / 2, c.shape[0] / 2), pos, ang, op=0)
        return [tuple(p) for p in co] + [tuple(co[0])]

    def _filled(self, cv, rgba, cx, cy, reveal, settle, op):
        """Text whose letters are filled with product photos, settling to solid colour."""
        if reveal <= 0 or op <= 0: return
        h, w = rgba.shape[:2]
        tex = cv2.resize(self.store[150:900, 200:2300], (w, h), interpolation=cv2.INTER_AREA).astype(np.float32)
        solid = np.array(rgba[..., :3], np.float32)
        base = np.array(rgba[..., :3], np.float32)
        tex_b = np.clip(tex * 2.0 + 70, 0, 255)
        col = solid * settle + tex_b * (1 - settle)
        out = np.dstack([np.clip(col, 0, 255), rgba[..., 3]]).astype(np.uint8)
        xs = np.arange(w, dtype=np.float32)
        edge = eo3(reveal) * (w + 120 * self.s) - 60 * self.s
        m = np.clip((edge - xs) / (60 * self.s) + 0.5, 0, 1)
        out[..., 3] = (out[..., 3] * m[None, :]).astype(np.uint8)
        put(cv, out, cx, cy, op)


# ---------------------------------------------------------------- driver
def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--preview", action="store_true")
    ap.add_argument("--final", action="store_true")
    ap.add_argument("--still", type=float, nargs="*")
    ap.add_argument("--sheet", action="store_true")
    ap.add_argument("--vo", default=str(ROOT / "assets/voice/vo_takeA.wav"))
    a = ap.parse_args()
    if a.final: w, h, fps = 1920, 1080, 60
    else: w, h, fps = 960, 540, 30
    film = Film(w, h)
    out = ROOT / "out"
    if a.still:
        for t in a.still:
            Image.fromarray(film.frame(t)).save(out / f"still_{t:05.2f}.png")
        return
    if a.sheet:
        ts = [0.05, 0.2, 0.5, 1.2, 2.2, 2.8, 3.1, 4.0, 4.6, 5.8, 6.2, 6.9, 7.6, 8.5, 9.6, 11.0, 12.2, 13.0, 14.0, 15.4, 16.2, 17.0, 18.0, 20.0]
        cols = 4
        tiles = [cv2.resize(film.frame(t), (480, 270)) for t in ts]
        rows = [np.hstack(tiles[i:i + cols]) for i in range(0, len(ts), cols)]
        Image.fromarray(np.vstack(rows)).save(out / "contact_sheet.png")
        return
    name = "hinekst_final_silent.mp4" if a.final else "hinekst_preview.mp4"
    ff = imageio_ffmpeg.get_ffmpeg_exe()
    cmd = [ff, "-y", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", f"{w}x{h}", "-r", str(fps), "-i", "-",
           "-i", a.vo, "-c:v", "libx264", "-preset", "medium", "-crf", "17" if a.final else "23",
           "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "192k", "-shortest", str(out / name)]
    p = subprocess.Popen(cmd, stdin=subprocess.PIPE)
    n = int(DUR * fps)
    for i in range(n):
        p.stdin.write(film.frame(i / fps).tobytes())
        if i % 60 == 0: print(f"{i}/{n}", flush=True)
    p.stdin.close(); p.wait()
    print("done", out / name)

if __name__ == "__main__":
    main()
