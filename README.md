# Audiora

Professional Windows audio platform by **Roujan Digital Agency**: a per-channel mixer for gaming and
daily use, a low-latency recording studio, and the official software for the **R audio interface**.

Proprietary — all rights reserved.

## What works today

* **Engine** (`audiora-svc`): any number of input and output devices, each on its own clock and
  bridged without clicks; strip chain denoise → gate → EQ → compressor → fader; bus limiter; shared,
  low-latency and **exclusive** WASAPI (3 ms periods); per-application volumes; survives devices
  disappearing; presets and session restore; recorder to multichannel 32-bit float WAV (RF64, crash-safe).
* **App** (`app/`): Svelte 5 + Tauri 2 — Play skin (mixer, application volumes) and Studio skin (EQ
  curve editor, dynamics, recorder, routing matrix, interface panel, system health), live meters at 30 Hz.
* **Control API** over a loopback WebSocket / TCP — see [docs/CONTROL-API.md](docs/CONTROL-API.md).
* **R Interface**: control layer plus a clearly-labelled **simulator**; licence system with a vendor
  tool.

## What is designed but not built (and why)

| Item | Blocked on |
|---|---|
| Virtual audio driver (Game/Chat/Media/Aux endpoints, per-app routing) | Windows Driver Kit + a spare test machine — [design](drivers/virtual-audio/DESIGN.md) |
| ASIO driver | Steinberg licence agreement — [design](drivers/asio/DESIGN.md) |
| Real R Interface backend | Hardware/firmware documentation — [R-INTERFACE.md](docs/R-INTERFACE.md) |
| Signed installer and drivers | Code-signing certificate, Hardware Dashboard account |

Full picture: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) and [docs/ROADMAP.md](docs/ROADMAP.md).

## Try it

```powershell
cargo test                                                  # engine and tools
cargo run -p audiora-cli -- devices                         # list audio devices
cargo run -p audiora-svc -- --no-input --tone 440 --sim-r   # engine + control API (quiet test tone)

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

### Package a portable build

```powershell
powershell -ExecutionPolicy Bypass -File scripts\package.ps1
```

## Requirements

Rust (stable, MSVC), Node 22+, Windows 10/11 with WebView2. Kernel driver work additionally needs the
Windows Driver Kit — `scripts\check-driver-env.ps1` tells you what is missing.
