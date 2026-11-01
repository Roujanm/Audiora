"""Synthesises the sound bed for the intro (no dependencies). Writes assets/intro_audio_3d.wav."""
import math, random, struct, wave, os

SR, DUR = 44100, 15.0
N = int(SR * DUR)
L = [0.0] * N
R = [0.0] * N
random.seed(7)


def add(t0, samples, pan=0.0, gain=1.0):
    i0 = int(t0 * SR)
    gl, gr = gain * (1 - max(pan, 0)), gain * (1 + min(pan, 0))
    for i, s in enumerate(samples):
        j = i0 + i
        if 0 <= j < N:
            L[j] += s * gl
            R[j] += s * gr


def env(i, n, a, d_pow):
    t = i / n
    return min(1.0, i / (a * SR)) * (1 - t) ** d_pow


def boom(freq, dur, drop=0.5):
    n = int(dur * SR)
    ph, out = 0.0, []
    for i in range(n):
        f = freq * (1 - drop * (i / n) ** 0.4)
        ph += 2 * math.pi * f / SR
        out.append(math.sin(ph) * env(i, n, 0.003, 3.0))
    return out


def noise_burst(dur, lp=0.15, d_pow=4.0):
    n = int(dur * SR)
    y, out = 0.0, []
    for i in range(n):
        y += (random.uniform(-1, 1) - y) * lp
        out.append(y * env(i, n, 0.001, d_pow))
    return out


def riser(dur):
    n = int(dur * SR)
    y, ph, out = 0.0, 0.0, []
    for i in range(n):
        t = i / n
        y += (random.uniform(-1, 1) - y) * (0.03 + 0.5 * t * t)
        ph += 2 * math.pi * (180 + 1500 * t * t) / SR
        out.append((y * 0.9 + math.sin(ph) * 0.25) * t ** 1.6)
    return out


def ping(freq, dur):
    n = int(dur * SR)
    return [(math.sin(2 * math.pi * freq * i / SR) + 0.35 * math.sin(4 * math.pi * freq * i / SR)) * env(i, n, 0.002, 2.5)
            for i in range(n)]


def pad(freqs, dur):
    n = int(dur * SR)
    out = []
    for i in range(n):
        t = i / SR
        s = sum(math.sin(2 * math.pi * f * t) + 0.5 * math.sin(2 * math.pi * f * 2.003 * t) for f in freqs) / (len(freqs) * 1.5)
        a = min(1.0, t / 1.4)
        b = min(1.0, (dur - t) / 1.6)
        out.append(s * a * b)
    return out


add(0.0, pad([55.0, 82.41], 5.0), gain=0.35)              # room tone / drone under the first half
add(1.0, boom(62, 0.9), gain=0.5)                          # stem rises
add(0.95, noise_burst(0.8, 0.08, 2.5), gain=0.16)
add(2.0, ping(1174, 0.7), pan=0.3, gain=0.2)               # sphere lands
add(2.05, boom(90, 0.5, 0.3), gain=0.22)
for k in range(12):                                        # light sweep shimmer
    add(1.5 + k * 0.19, ping(2300 + 170 * k, 0.5), pan=-0.5 + 0.09 * k, gain=0.045)
add(3.5, riser(1.46), gain=0.42)                           # dive into the sphere
add(4.96, boom(46, 3.2, 0.35), gain=1.0)                   # cut / impact
add(4.96, noise_burst(1.4, 0.35, 3.0), gain=0.5)
add(5.1, pad([110.0, 164.81, 220.0, 277.18, 329.63], 9.6), gain=0.3)
for n, f in enumerate((587, 659, 784, 880, 988, 1175, 1319, 1568)):   # light front opening
    add(5.2 + n * 0.28, ping(f, 1.4), pan=-0.4 + 0.1 * n, gain=0.075)
for n in range(7):                                         # letters lock in
    add(7.1 + n * 0.17, ping(1568 + 90 * n, 0.9), pan=-0.5 + 0.17 * n, gain=0.09)
add(9.9, ping(2217, 2.4), pan=0.2, gain=0.09)
add(9.9, boom(70, 1.2, 0.2), gain=0.2)

for i in range(N):
    t = i / SR
    g = 1.0 if t < 14.0 else max(0.0, 15.0 - t)
    L[i] *= g
    R[i] *= g
peak = max(max(abs(x) for x in L), max(abs(x) for x in R)) or 1.0
scale = 0.89 / peak
os.makedirs(os.path.join(os.path.dirname(__file__), "assets"), exist_ok=True)
path = os.path.join(os.path.dirname(__file__), "assets", "intro_audio_3d.wav")
with wave.open(path, "wb") as w:
    w.setnchannels(2)
    w.setsampwidth(2)
    w.setframerate(SR)
    w.writeframes(b"".join(struct.pack("<hh", int(L[i] * scale * 32767), int(R[i] * scale * 32767)) for i in range(N)))
print("wrote", path)
