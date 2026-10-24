# Audiora — System Architecture

Owner: Roujan Digital Agency · Status: v0.1 (foundation) · Platform: Windows 10 22H2 / Windows 11 (x64)

## 1. Product scope

One audio platform, three experiences on a single engine:

| Edition | Audience | Core value |
|---|---|---|
| **Audiora Play** | Gamers, streamers, daily users | Per-app mixer (Game / Chat / Media / Aux / Mic), ChatMix, EQ, presets |
| **Audiora Studio** | Producers, podcasters, musicians | Low-latency I/O, multitrack recording, monitoring, routing matrix, plug-ins |
| **R Interface support** | Owners of the R audio interface | First-class driver, hardware control, zero-config recording |

Any third-party class-compliant device must also work (Tier 2); the R Interface is Tier 1
(see `R-INTERFACE.md`).

## 2. Design principles

1. **The audio thread never blocks.** No allocation, locks, syscalls or logging inside a
   callback. Control reaches DSP only via atomics and wait-free ring buffers.
2. **Drivers stay thin.** Kernel code moves bytes and clocks; all DSP lives in user mode where a
   crash cannot blue-screen the machine.
3. **Engine is a service, UI is a client.** The UI can crash or restart without a dropout.
4. **32-bit float everywhere internally**, 64-bit filter state, no hidden sample-rate conversion
   except where clock domains differ (§6).
5. **Latency is measured and reported**, not assumed (drivers report real offsets to hosts).
6. **One public local API.** The UI uses the same API third parties get.

## 3. System overview

```
 Apps / Games / Browser / DAWs
        │ WASAPI / MME / DirectSound             DAW (Cubase, Reaper, FL…)
        ▼                                                │ ASIO
┌──────────────────────────┐                  ┌──────────▼─────────────┐
│ Audiora Virtual Audio    │                  │ Audiora ASIO driver     │
│ (kernel, WaveRT)         │                  │ (user-mode COM DLL)     │
│ Game│Chat│Media│Aux│Mic  │                  └──────────┬─────────────┘
└────────────┬─────────────┘   shared-memory rings        │ shared memory
             └────────────────────┬──────────────────────┘
                                  ▼
        ┌──────────────────────────────────────────────────┐
        │              audiora-svc  (Windows service)      │
        │  HAL ─ Clock/Resampler ─ Mixer graph ─ Recorder  │
        │  Plug-in host (out-of-process) ─ Meters ─ Rules  │
        └───────┬───────────────────────────────┬──────────┘
          WASAPI / USB (R Interface)       Control API (named pipe + local WS)
                │                               │
          Hardware I/O                  Audiora App: Tauri 2 + Svelte
                                        (Play skin · Studio skin) · CLI · 3rd-party
```

## 4. Components

### 4.1 Drivers (`drivers/`)
| Driver | Type | Purpose | Status |
|---|---|---|---|
| `virtual-audio` | Kernel WDM/PortCls, WaveRT, derived from Microsoft SysVAD | Virtual playback/capture endpoints visible to every app | Planned (needs WDK) |
| `asio` | User-mode COM DLL (ASIO 2.x) | Low-latency access for DAWs, to the R Interface and virtual endpoints | Planned |
| `r-interface` | USB Audio Class 2.0 via Windows inbox `usbaudio2.sys` first; custom KMDF/UMDF streaming driver only if latency/feature goals demand | Hardware | Depends on firmware (see R-INTERFACE.md) |

Kernel-mode rules: C/C++ only, no DSP, no allocation in the stream path, Driver Verifier +
WinDbg on every build. `windows-drivers-rs` is not production-ready; revisit later.

### 4.2 Engine service `audiora-svc` (Rust)
- **HAL** (`audiora-hal`): one trait family over WASAPI shared, WASAPI exclusive, IAudioClient3
  low-latency shared, ASIO, and the R Interface backend. *Implemented: WASAPI shared render and
  capture, device enumeration.*
- **Mixer graph** (`audiora-core`): strips → buses, per-strip EQ/gain/balance/mute/solo,
  smoothed parameters, peak metering. *Implemented.* Next: sends, inserts, mono strips, N-channel
  buses, sidechain.
- **DSP** (`audiora-dsp`): biquads (RBJ). *Implemented.* Next: compressor, gate, limiter, de-esser,
  noise suppression (RNNoise-class), convolution/reverb, true-peak/LUFS meters.
- **Recorder**: audio thread → lock-free ring → writer thread → 32-bit float WAV/BWF, RF64 for
  files > 4 GB, crash-safe header patching, punch-in/out, take management.
- **Per-app routing**: map processes to strips (virtual endpoint + audio-session policy).
- **Plug-ins**: VST3 and CLAP, hosted in a sandbox process; crash of a plug-in never stops audio.

### 4.3 Control plane
- Transport: named pipe (ACL: current user + service) and a loopback WebSocket for the UI.
  Today: loopback TCP, JSON lines — see `CONTROL-API.md`.
- Schema: versioned, MessagePack/JSON via serde; parameter paths like `strip/3/eq/1/gain_db`.
- Meters stream at 30–60 Hz, parameters are request/response + change events.
- Persistence: profiles/presets as JSON in `%AppData%\Roujan\Audiora`.

### 4.4 Application (`app/`)
- **Tauri 2 + Svelte 5 (TypeScript)**. Svelte's compiled, fine-grained reactivity suits 60 fps
  meters and faders without a virtual DOM; Tauri keeps the footprint small and native.
- Two skins on one component library: **Play** (clean per-channel mixer) and **Studio**
  (routing matrix, tracks, meters, spectrum). Canvas/WebGL for meters and spectrum.
- UI never touches audio; it only speaks the control API.

## 5. Latency model

| Path | Target round-trip | Notes |
|---|---|---|
| Play, shared mode through virtual endpoints | 10–25 ms | Fine for games/chat |
| IAudioClient3 low-latency shared | ~5–10 ms | Windows 10+ |
| Studio, ASIO + R Interface | ≤ 4 ms @ 96 kHz / 64 frames | Requires R driver quality |
| Hardware direct monitoring (R) | ~0 ms | Preferred for recording |

Every path reports its actual buffer + driver + converter latency to the UI.

## 6. Clocking

Each device is its own clock domain. Rules: the R Interface (when present) is master; other
devices and virtual endpoints are slaves with adaptive resampling (drift-tracking sinc SRC,
PI-controlled ring-buffer fill). No silent dropouts: underrun/overrun counters are exposed in the
UI and logged.

## 7. Security, signing, distribution

- Kernel drivers: Microsoft Partner Center attestation signing; requirements (EV vs standard
  cert) must be verified against current Microsoft docs before purchasing certificates.
- Development: test-signing on a dedicated machine only. Dev work on a primary machine risks
  blue screens and conflicts with other virtual drivers (Voicemeeter, Sonar).
- Service runs least-privilege; IPC is ACL-restricted; updates are signed; installer is
  WiX/MSIX with driver pre-staging via `pnputil`.
- Licensing for ASIO: obtain Steinberg's ASIO SDK licence terms before shipping a driver.

## 8. Technology choices

| Concern | Choice | Why |
|---|---|---|
| Kernel drivers | C/C++ (WDK, SysVAD base) | Only fully supported path |
| Engine / service | Rust | Memory safety in real-time code, strong concurrency, easy tests |
| Audio I/O (user mode) | WASAPI via `windows` crate, ASIO | Native, no extra runtime |
| UI | Tauri 2 + Svelte 5 + TypeScript | Light, fast, consistent with a brand-led product |
| IPC | Named pipe + loopback WS, serde | Simple, debuggable, public API |
| Build | Cargo workspace + pnpm + CMake/MSBuild (drivers) | Per-language native tooling |

## 9. Repository layout

```
Audiora/
├─ crates/
│  ├─ audiora-dsp/    biquads, dB helpers, linear resampler (done)
│  ├─ audiora-core/   mixer, param paths, SPSC ring  (done)
│  ├─ audiora-hal/    WASAPI backend                 (done: enumerate, render, capture)
│  ├─ audiora-cli/    `audiora devices|tone`         (done)
│  ├─ audiora-svc/    engine + JSON-lines control API (done, runs as a console app; Windows service wrapper next)
│  └─ audiora-rec/    recorder                       (next)
├─ drivers/           virtual-audio, asio, r-interface (planned)
├─ app/               Tauri + Svelte UI              (planned)
└─ docs/
```

## 10. Top risks

1. Kernel driver quality and signing lead time → start driver work early, in parallel.
2. R Interface firmware/USB descriptors unknown → see open questions in `R-INTERFACE.md`.
3. Clock drift between devices → solved in the engine, needs long-run soak tests.
4. Conflicts with existing virtual audio software on user machines → detection + guidance in
   the installer.
