"""Generate the Turkish voice-over with Edge TTS (tr-TR-EmelNeural): one file per line,
then place each line at its start time on a timeline with ffmpeg."""
import asyncio, subprocess, sys
from pathlib import Path
import edge_tts
import imageio_ffmpeg

ROOT = Path(r"C:\Users\rouja\Documents\GitHub\Audiora\hinekst-video")
OUT = ROOT / "assets" / "voice"
FFMPEG = imageio_ffmpeg.get_ffmpeg_exe()
VOICE = "tr-TR-EmelNeural"
TOTAL = 21.2

LINES = [  # (start seconds, text)
    (0.50, "Bir merhaba…"),
    (2.67, "…bir toplulukla başlar."),
    (5.40, "Tek dokunuş…"),
    (6.60, "ve takımın seste."),
    (10.60, "Kendi sunucun, kendi kuralların."),
    (13.50, "Merhaba'dan…"),
    (15.10, "Sonrakine."),
    (17.50, "HiNekst."),
]
TAKES = {
    "A": dict(rate="+0%", pitch="+0Hz", last="HiNekst."),
    "B": dict(rate="-6%", pitch="-2Hz", last="Hay Nekst."),
}


async def gen(text, path, rate, pitch):
    await edge_tts.Communicate(text, VOICE, rate=rate, pitch=pitch).save(str(path))


def dur(path):
    r = subprocess.run([FFMPEG, "-i", str(path), "-f", "null", "-"], capture_output=True, text=True)
    for ln in r.stderr.splitlines():
        if "time=" in ln:
            t = ln.split("time=")[-1].split()[0]
            h, m, s = t.split(":")
            return int(h) * 3600 + int(m) * 60 + float(s)
    return 0.0


async def main():
    for take, cfg in TAKES.items():
        d = OUT / f"take{take}"
        d.mkdir(parents=True, exist_ok=True)
        files = []
        for i, (start, text) in enumerate(LINES):
            if i == len(LINES) - 1:
                text = cfg["last"]
            p = d / f"line{i+1}.mp3"
            await gen(text, p, cfg["rate"], cfg["pitch"])
            w = d / f"line{i+1}.wav"   # silence-trimmed copy used on the timeline
            subprocess.run([FFMPEG, "-y", "-i", str(p), "-af",
                            "silenceremove=start_periods=1:start_threshold=-45dB,"
                            "areverse,silenceremove=start_periods=1:start_threshold=-45dB,areverse",
                            "-ar", "48000", str(w)], check=True, capture_output=True)
            files.append((start, w, text))
        cmd = [FFMPEG, "-y"]
        for _, p, _ in files:
            cmd += ["-i", str(p)]
        parts, labels = [], []
        for i, (start, _, _) in enumerate(files):
            ms = int(start * 1000)
            parts.append(f"[{i}:a]aresample=48000,aformat=channel_layouts=mono,adelay={ms}:all=1[a{i}]")
            labels.append(f"[a{i}]")
        fc = ";".join(parts) + ";" + "".join(labels) + f"amix=inputs={len(files)}:normalize=0,apad=whole_dur={TOTAL}[out]"
        cmd += ["-filter_complex", fc, "-map", "[out]", "-t", str(TOTAL), "-ar", "48000", str(OUT / f"vo_take{take}.wav")]
        subprocess.run(cmd, check=True, capture_output=True)
        print(f"Take {take}:")
        for i, (start, p, text) in enumerate(files):
            dd = dur(p)
            nxt = files[i + 1][0] if i + 1 < len(files) else TOTAL
            flag = "  !! sonraki satıra taşıyor" if start + dd > nxt else ""
            print(f"  {start:5.2f}s  {dd:4.2f}s  bitiş {start+dd:5.2f}  '{text}'{flag}")


asyncio.run(main())
