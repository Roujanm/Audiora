# Audiora — System Architecture

Owner: Roujan Digital Agency · Status: v0.1 · Platform: Windows 10 22H2 / Windows 11 (x64)

## 1. Product scope

One audio platform, three experiences on a single engine:

| Edition | Audience | Core value |
|---|---|---|
| **Audiora Play** | Gamers, streamers, daily users | Per-app mixer (Game / Chat / Media / Aux / Mic), ChatMix, EQ, presets |
| **Audiora Studio** | Producers, podcasters, musicians | Low-latency I/O, multitrack recording, monitoring, routing matrix, processing |
| **R Interface support** | Owners of the R audio interface | First-class driver, hardware control, zero-config recording |

Any third-party class-compliant device must also work (Tier 2); the R Interface is Tier 1
(see `R-INTERFACE.md`).

## 2. Design principles

1. **The audio thread never blocks.** No allocation, locks, syscalls or logging inside a
   callback. Control reaches DSP only via atomics and wait-free ring buffers.
2. **Drivers stay thin.** Kernel code moves bytes and clocks; all DSP lives in user mode where a
   crash cannot blue-screen the machine.
3. **Engine is a separate process, UI is a client.** The UI can crash or restart without a dropout.
4. **32-bit float everywhere internally**, 64-bit filter state; sample-rate conversion happens only
   where two clock domains meet (§6).
5. **Latency is measured and reported**, not assumed.
6. **One public local API.** The UI uses the same API third parties get.
7. **Honesty about hardware.** Anything that stands in for real hardware (the R Interface
   simulator) is labelled as such in the API and the UI.

## 3. System overview

```
 Apps / Games / Browser                     DAW (Cubase, Reaper, FL…)
        │ WASAPI / MME / DirectSound                    │ ASIO
        ▼                                               ▼
┌──────────────────────────┐                  ┌────────────────────────┐
│ Audiora Virtual Audio    │ (designed)       │ Audiora ASIO driver    │ (designed)
│ kernel, WaveRT endpoints │                  │ user-mode COM DLL      │
└────────────┬─────────────┘                  └───────────┬────────────┘
             │ shared-memory rings                         │ device lease
             ▼                                             ▼
        ┌─────────────────────────────────────────────────────────────┐
        │  audiora-svc  (per-user background process)       [built]   │
        │  capture threads ─ring─ DriftResampler ─┐                   │
        │  tone generator ────────────────────────┼─► Mixer           │
        │   strips: denoise → gate → EQ → comp →  │   buses: fader →  │
        │           fader/balance → bus sends     │          limiter  │
        │  Recorder tap ─ring─ disk thread        └─► master device   │
        │  extra buses ─ring─ DriftResampler ─► other devices         │
        │  Presets · Licence · Hardware control (R / simulator)       │
        └───────┬─────────────────────────────────────────┬───────────┘
          WASAPI shared / IAudioClient3                    │ JSON control API
                │                                   WebSocket (Origin-checked) + TCP lines
          Audio hardware                                   │
                                          Audiora app: Tauri 2 + Svelte 5   [built]
                                          Play skin · Studio skin · CLI · 3rd-party
```

### Why a per-user process and not a Windows service
Services run in session 0, which does not see the signed-in user's default audio devices and is
subject to microphone-privacy rules that apply to the user's session. Voicemeeter and Sonar both run
their engines in the user session for the same reason. The engine therefore starts at sign-in
(`scripts/install-user.ps1 -Autostart`) or when the app launches. A real service is only worth adding
for driver installation/update, where administrator rights are needed.

## 4. Components

### 4.1 Drivers (`drivers/`)
| Driver | Type | Purpose | Status |
|---|---|---|---|
| `virtual-audio` | Kernel WDM/PortCls, WaveRT, derived from Microsoft SysVAD | Virtual playback/capture endpoints visible to every app | Designed (`drivers/virtual-audio/DESIGN.md`); needs WDK + test machine |
| `asio` | User-mode COM DLL (ASIO 2.x), Rust | Low-latency access for DAWs | Designed (`drivers/asio/DESIGN.md`); needs Steinberg licence decision |
| `r-interface` | USB Audio Class 2.0 via inbox `usbaudio2.sys` first; custom KMDF/UMDF streaming driver only if latency/feature goals demand | Hardware | Depends on firmware (see R-INTERFACE.md) |

Kernel-mode rules: C/C++ only, no DSP, no allocation in the stream path, Driver Verifier +
WinDbg on every build. `windows-drivers-rs` is not production-ready; revisit later.

### 4.2 Engine `audiora-svc` (Rust)
- **HAL** (`audiora-hal`): WASAPI, event-driven, render and capture, in three modes — shared,
  low-latency shared (`IAudioClient3` minimum period) and **exclusive** (native float or
  16/24/32-bit integer format, device minimum period, converted to/from f32 at the edge). Audio
  threads join the MMCSS "Pro Audio" class. Also lists per-application audio sessions and sets their
  volume/mute (what the Windows volume mixer does). Planned behind the same types: ASIO, R
  Interface native backend.
- **Graph** (`audiora-svc/engine.rs`): any number of capture devices become strips; any number of
  output devices become buses. The first output is the master that clocks the mixer. Every other
  device is its own clock domain, bridged by a `DriftResampler`. A device that disappears (unplugged,
  taken exclusively by another application) does not stop the engine: that path goes silent, a
  `device_errors` counter ticks, and the device is reopened automatically when it returns with the
  same rate and channel count. Capture strips start unrouted so a microphone never feeds back by
  default.
- **Mixer** (`audiora-core`): strip chain *denoise → gate → 4-band EQ → compressor → fader/balance*,
  bus chain *sum → fader → brick-wall limiter*. Parameters ramp per block (no zipper noise),
  every value is addressable by path, meters include gain reduction.
- **DSP** (`audiora-dsp`): RBJ biquads, gate, compressor with soft knee, look-ahead limiter with a
  provable ceiling, RNNoise-port denoiser (48 kHz), linear resampler. Not yet: de-esser,
  convolution/reverb, LUFS / true-peak meters.
- **Recorder** (`audiora-rec`): audio thread → wait-free ring → disk thread → 32-bit float WAV,
  extensible format for 3+ channels, RF64 beyond 4 GB, header re-patched every second so a crash
  keeps everything but the last second. Not yet: punch-in/out, take management, input monitoring.
- **Presets**: one JSON file per preset, atomic writes, name validation (no path traversal); the last
  session is restored on start.
- **Hardware control** (`audiora-device`): capability description + parameters + meters for any
  interface. Ships a labelled simulator; the real USB protocol is pending.
- **Licence** (`audiora-license`): offline Ed25519-signed files, optional lock to an R Interface
  serial; vendor tool issues them. Edition gating is a product decision not yet made.
- **Not built (needs the driver):** per-application routing.
- **Not built (large):** VST3 / CLAP hosting in a sandbox process.

### 4.3 Control plane
- Transports: loopback WebSocket (Origin allow-list + Host check, 16-client cap) for the app, and
  loopback TCP JSON-lines for tools. Both bind 127.0.0.1 only. Before release the TCP transport
  becomes a named pipe with an ACL; any local process can still talk to the engine today, which
  matches what Voicemeeter's remote API allows.
- Schema: JSON, parameter paths like `strip/3/eq/1/gain_db` — see `CONTROL-API.md`.
- Meters are pushed at up to 60 Hz after `subscribe`; everything else is request/response.
- Persistence: `%APPDATA%\Roujan\Audiora\presets`, `license.json`.

### 4.4 Application (`app/`)
- **Tauri 2 + Svelte 5 (TypeScript)**. The shell (`app/src-tauri`) starts the engine if it is not
  running and asks it to shut down cleanly (recordings finalized, session saved) when the app exits
  — but only if the app started it.
- **Play skin**: the mixer — strips, buses, presets. **Studio skin** adds EQ & dynamics, recording,
  routing matrix, interface control and system health.
- All state comes from the engine; the UI keeps no audio logic except display maths that mirrors the
  engine (EQ curve, meter ballistics), covered by tests against the same expectations as the Rust.

## 5. Latency model

| Path | Measured / target | Notes |
|---|---|---|
| Shared mode, default period | ~10 ms device period (measured: 480 frames @ 48 kHz) | Fine for games/chat |
| `IAudioClient3` low-latency shared | 2 ms period on a virtual 96 kHz device (192 frames); Bluetooth headset stays at 10 ms | Hardware decides the minimum |
| WASAPI exclusive | 3 ms period (144 frames @ 48 kHz, 24-bit) on onboard and HDMI audio, capture and render, no underruns in a release build | Needs the device to itself |
| Added by the engine | one producer period + one consumer period + 50 % margin per device hop | `DriftResampler` target fill |
| Studio, ASIO + R Interface | target ≤ 4 ms @ 96 kHz / 64 frames | Needs the R driver |
| Hardware direct monitoring (R) | ~0 ms | Preferred for recording |

## 6. Clocking

Each device is its own clock domain. A producer thread writes frames at its native rate into a
wait-free ring; the consumer reads through a 4-point Catmull-Rom interpolator whose ratio is trimmed
(≤ ±0.5 %, slow proportional control on the filtered ring fill) so the fill level holds its target.
The same component handles different nominal rates (16/44.1/48/96 kHz) and clock drift.
Verified by simulation at ±400 ppm over 60 s (no underruns, no drops, fill stays bounded, output
continuous) and live with two inputs and two outputs at 48 and 96 kHz.

## 7. Security, signing, distribution

- Local control API: loopback only; WebSocket upgrades are refused for any Origin outside the app's
  own and for any Host other than the loopback names (cross-site WebSocket hijacking and DNS
  rebinding).
- Kernel drivers: Microsoft Hardware Dashboard signing; the certificate type required (EV vs
  standard) must be verified against current Microsoft documentation before buying — sources
  disagreed. Development uses test-signing on a dedicated machine only.
- Licences: only the public key ships; the private key stays offline with the vendor tool.
- Packaging: `scripts/package.ps1` builds a portable zip with a SHA-256; the Tauri bundle targets an
  NSIS per-user installer. Code-signing the executables needs a certificate not yet purchased.
- ASIO: Steinberg's licence terms must be settled before any ASIO code is written.

## 8. Technology choices

| Concern | Choice | Why |
|---|---|---|
| Kernel drivers | C/C++ (WDK, SysVAD base) | Only fully supported path |
| Engine | Rust | Memory safety in real-time code, strong concurrency, easy tests |
| Audio I/O (user mode) | WASAPI via `windows` crate | Native, no extra runtime |
| Denoiser | `nnnoiseless` (RNNoise port) | BSD-licensed, pure Rust |
| UI | Tauri 2 + Svelte 5 + TypeScript | Light, fast, fine-grained reactivity for meters |
| IPC | Loopback WebSocket + JSON lines | Simple, debuggable, public API |
| Licences | Ed25519 (`ed25519-dalek`) | Small, offline-verifiable |

## 9. Repository layout

```
Audiora/
├─ crates/
│  ├─ audiora-dsp/      biquads, gate, compressor, limiter, denoiser, resampler
│  ├─ audiora-core/     mixer, parameter paths, rings, DriftResampler
│  ├─ audiora-hal/      WASAPI render/capture (+ IAudioClient3)
│  ├─ audiora-rec/      WAV/RF64 writer + disk thread
│  ├─ audiora-device/   hardware control abstraction + R Interface simulator
│  ├─ audiora-license/  Ed25519 licence files + vendor tool
│  ├─ audiora-svc/      the engine process + control API
│  └─ audiora-cli/      `audiora devices | tone`
├─ app/                 Svelte 5 UI; app/src-tauri = Tauri 2 shell
├─ drivers/             virtual-audio and asio designs; r-interface pending firmware docs
├─ scripts/             package, per-user install, driver environment check
└─ docs/
```

## 10. Status at a glance

| Area | State |
|---|---|
| Engine, mixer, DSP, recorder, presets, control API, exclusive mode, per-app volumes, device-loss recovery | Built and tested |
| Svelte app + Tauri shell | Built; UI verified live against the engine; Tauri shell compiles |
| R Interface | Simulator only — real protocol unknown |
| Virtual audio driver | Designed — blocked: WDK missing, no safe test machine |
| ASIO driver | Designed — blocked: Steinberg licence decision |
| Per-app routing | Blocked on the virtual driver |
| VST3 / CLAP hosting, LUFS meters, convolution | Not started |
| Installer signing, updater, telemetry | Not started (needs certificates and hosting) |

## 11. Top risks

1. Kernel driver quality and signing lead time → start driver work early, on a dedicated machine.
2. R Interface firmware / USB descriptors unknown → see open questions in `R-INTERFACE.md`.
3. Long-run clock behaviour → soak tests (1 h+) still to run.
4. Conflicts with existing virtual audio software (Voicemeeter, Sonar) → detection and guidance in
   the installer.
5. Undocumented Windows per-app routing API → isolate behind one module with a version guard.
