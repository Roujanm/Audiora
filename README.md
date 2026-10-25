# Audiora

Professional Windows audio platform by **Roujan Digital Agency**: a per-channel mixer for gaming and
daily use, a low-latency recording studio, and the official software for the **R audio interface**.

Proprietary — all rights reserved.

## What works today

* **Engine** (`audiora-svc`): any number of input and output devices, each on its own clock and
  bridged without clicks; per-channel chain boost → denoise → gate → 6-band EQ → Smart Volume →
  compressor → fader; bus limiter; shared, low-latency and **exclusive** WASAPI (3 ms periods);
  survives devices disappearing; presets and session restore; recorder to multichannel 32-bit float
  WAV (RF64, crash-safe).
* **Sonar-style channels**: Game, Chat, Media, Aux and Mic. Programs that make sound are detected
  automatically; drag one onto a channel and Windows plays it into that channel's virtual output while
  Audiora mixes it (per-app output routing + loopback, live, no driver of ours). Per-channel presets
  with favourites, per-game profiles that load by themselves when the game starts, per-program
  volumes, Smart Volume, ClearCast noise cancellation, gate, compressor. Works without any driver or cable — see below.
* **Studio**: multitrack recorder wired to the whole system — record the processed microphone,
  any channel or the master on separate tracks, overdub over earlier tracks, see waveforms, split and
  move clips, mute/solo/volume per track, export the mix. Everything autosaves.
* **App** (`app/`): Svelte 5 + Tauri 2 with a hardware-style interface (knobs, machined faders, LED
  meters, five accent colours): the channel mixer with Master and the app list at the left, one page
  per channel, recorder with takes list, interface panel, Setup. Live meters at 30 Hz, master LUFS
  (BS.1770), live spectrum behind the EQ, click-to-type values and a Ctrl+K command palette.
* **Control API** over a loopback WebSocket / TCP — see [docs/CONTROL-API.md](docs/CONTROL-API.md).
* **R Interface**: control layer plus a clearly-labelled **simulator**; licence system with a vendor
  tool.

## What is designed but not built (and why)

| Item | Blocked on |
|---|---|
| Audiora's own virtual audio driver (replaces the third-party cables; adds a real virtual microphone) | Windows Driver Kit + a spare test machine — [design](drivers/virtual-audio/DESIGN.md) |
| ASIO driver | Steinberg licence agreement — [design](drivers/asio/DESIGN.md) |
| Real R Interface backend | Hardware/firmware documentation — [R-INTERFACE.md](docs/R-INTERFACE.md) |
| Signed installer and drivers | Code-signing certificate, Hardware Dashboard account |

Full picture: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) and [docs/ROADMAP.md](docs/ROADMAP.md).

## Try it

```powershell
cargo test                                                  # engine and tools
cargo run -p audiora-cli -- devices                         # list audio devices
cargo run -p audiora-svc -- --tone 440 --sim-r              # engine + control API (channel layout)
cargo run -p audiora-cli -- route PID default               # per-app output, e.g. undo a route by hand

cd app
npm install
npm run dev                                                 # UI at http://localhost:5173 (engine must be running)
npm test                                                    # UI maths tests
```

`audiora-svc` flags are listed in [docs/CONTROL-API.md](docs/CONTROL-API.md). The tone strip starts at
-30 dB; lower it before wearing headphones if in doubt.

### Desktop app

```powershell
cd app
npx tauri dev      # opens the window and starts the engine for you
npx tauri build    # NSIS per-user installer (unsigned until a certificate exists)
```

### Install for this user (desktop shortcut)

```powershell
powershell -ExecutionPolicy Bypass -File scripts\package.ps1          # builds dist\Audiora-<version>-win-x64
powershell -ExecutionPolicy Bypass -File dist\Audiora-0.1.0-win-x64\install-user.ps1 -Shortcuts
```

Installs to `%LOCALAPPDATA%\Roujan\Audiora` (no administrator rights) and puts an **Audiora**
shortcut on the desktop and in the Start menu. `-Uninstall` removes both.

### Package a portable build

```powershell
powershell -ExecutionPolicy Bypass -File scripts\package.ps1
```

### How apps reach the channels (no driver needed)

When you drag a program onto a channel, Windows is told to play that program into an output you do
not listen to — automatically an unused digital/optical port or a monitor's HDMI audio — and Audiora
captures the program's sound directly (per-process loopback) into the channel. Nothing is installed.
Pick the output in **Setup** if the automatic one has speakers attached. A channel can instead use a
virtual output (a cable, later Audiora's own driver); channels never accept your real speakers, which
would echo forever. Programs that open a fixed audio device instead of the Windows default need their
audio restarted once after routing.

## Requirements

Rust (stable, MSVC), Node 22+, Windows 10/11 with WebView2. Kernel driver work additionally needs the
Windows Driver Kit — `scripts\check-driver-env.ps1` tells you what is missing.
