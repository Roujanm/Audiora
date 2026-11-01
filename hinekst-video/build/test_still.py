import sys
sys.path.insert(0, r"C:\Users\rouja\Documents\GitHub\Audiora\hinekst-video\build")
import numpy as np
from PIL import Image
from common import *

w, h = W16, H16
f = stage(w, h, 0.4)
im = Image.fromarray(f).convert("RGBA")
t = text_img("HiNekst", 220)
im.alpha_composite(t, ((w - t.width) // 2, (h - t.height) // 2 - 40))
f = np.array(im.convert("RGB"))
y = h // 2 + 120
x0 = (w - t.width) // 2
curve = [(x0 + i, y + 22 * np.sin(i / t.width * np.pi)) for i in range(0, t.width, 8)]
f = draw_light_line(f, curve, head=0.8, tail=0.0, width=6)
Image.fromarray(f).save(r"C:\Users\rouja\Documents\GitHub\Audiora\hinekst-video\out\test_still.png")
print("saved")
