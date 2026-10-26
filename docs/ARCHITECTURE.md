# Audiora — System Architecture

Owner: Roujan Digital Agency · Status: v0.1 · Platform: Windows 10 22H2 / Windows 11 (x64)

## 1. Product scope

One audio platform, three experiences on a single engine (the app is one professional interface;
what each edition unlocks is a licensing decision, not a mode switch):

| Edition | Audience | Core value |
|---|---|---|
| **Audiora Play** | Gamers, streamers, daily users | Per-app channels (Game / Chat / Media / Aux / Mic) with drag-and-drop routing, per-channel EQ and presets, per-game profiles |
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
                                          Channel mixer · CLI · 3rd-party
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
| `virtual-audio` | Kernel WDM/PortCls, WaveRT, derived from Microsoft SysVAD | Virtual playback/capture endpoints visible to every app | Designed (`drivers/virtual-audio/DESIGN.md`); needs WDK + test machine. Until then Audiora uses third-party virtual cables (section 4.2, *Channels*) |
| `asio` | User-mode COM DLL (ASIO 2.x), Rust | Low-latency access for DAWs | Designed (`drivers/asio/DESIGN.md`); needs Steinberg licence decision |
| `r-interface` | USB Audio Class 2.0 via inbox `usbaudio2.sys` first; custom KMDF/UMDF streaming driver only if latency/feature goals demand | Hardware | Depends on firmware (see R-INTERFACE.md) |

Kernel-mode rules: C/C++ only, no DSP, no allocation in the stream path, Driver Verifier +
WinDbg on every build. `windows-drivers-rs` is not production-ready; revisit later.

### 4.2 Engine `audiora-svc` (Rust)
- **HAL** (`audiora-hal`): WASAPI, event-driven, render and capture, in three modes — shared,
  low-latency shared (`IAudioClient3` minimum period) and **exclusive** (native float or
  16/24/32-bit integer format, device minimum period, converted to/from f32 at the edge). Audio
  threads join the MMCSS "Pro Audio" class. Also lists per-application audio sessions and sets their
  volume/mute (what the Windows volume mixer does), **loopback capture of a render device**, and
  **per-application default output** (`policy.rs`: the `AudioPolicyConfig` factory behind Settings >
  Sound > Volume mixer, with both known interface ids and a read-back check). A *switchable* render
  stream (class activation, automatic format conversion) is what routing-aware programs use; Windows
  moves such a stream live when its output changes. Planned behind the same types: ASIO, R
  Interface native backend.
- **Graph** (`audiora-svc/engine.rs`): any number of input devices — microphones, or loopback taps of
  a render device — become strips; any number of output devices become buses. The first output is the master that clocks the mixer. Every other
  device is its own clock domain, bridged by a `DriftResampler`. A device that disappears (unplugged,
  taken exclusively by another application) does not stop the engine: that path goes silent, a
  `device_errors` counter ticks, and the device is reopened automatically when it returns with the
  same rate and channel count. Capture strips start unrouted so a microphone never feeds back by
  default. A loopback tap delivers no packets while its device is idle, so the capture thread turns
  wall-clock time into silence (after 30 ms of quiet) and the mixer never sees a ring run dry.
- **Mixer** (`audiora-core`): strip chain *volume boost → denoise → gate → 6-band EQ → Smart Volume →
  compressor → fader/balance*,
  bus chain *sum → fader → brick-wall limiter*. Parameters ramp per block (no zipper noise),
  every value is addressable by path, meters include gain reduction.
- **DSP** (`audiora-dsp`): RBJ biquads, gate, compressor with soft knee, look-ahead limiter with a
  provable ceiling, Smart Volume leveler (slow AGC with a noise-floor guard), RNNoise-port denoiser
  (48 kHz), linear resampler, BS.1770 loudness meter,
  spectrum analyser. Not yet: de-esser, convolution/reverb, true-peak / LRA meters.
- **Analysis** (`audiora-svc/analysis.rs`, off the audio thread): programme loudness of the master bus
  per ITU-R BS.1770-4 / EBU R128 — K-weighting (coefficients verified against the standard), momentary
  400 ms, short-term 3 s, gated integrated, checked against the EBU Tech 3341 reference tone — and a
  96-band FFT spectrum of one selectable signal. The audio callback only copies blocks into wait-free
  rings.
- **Recorder** (`audiora-rec`): audio thread → wait-free ring → disk thread → 32-bit float WAV,
  extensible format for 3+ channels, RF64 beyond 4 GB, header re-patched every second so a crash
  keeps everything but the last second. Not yet: punch-in/out, take management, input monitoring.
- **Presets**: one JSON file per preset, atomic writes, name validation (no path traversal); the last
  session is restored on start and autosaved within two seconds of any change (a crash or forced
  close loses nothing). **Channel presets** store only the sound-shaping settings of one
  channel (EQ, boost, Smart Volume, noise suppression, gate, compressor — never fader, mute or
  routing): built-in sets per channel plus the user's own, with favourites. **Game profiles** name a
  preset per program and channel; the engine applies it the moment that program starts making sound.
- **Channels** (`audiora-svc/channels.rs`): Game, Chat, Media and Aux are application mixes, Mic is the
  microphone. By default a channel needs no device at all (*direct capture*): Windows is told per
  application to play into a *parking output* nobody listens to (an unused S/PDIF port or monitor HDMI
  audio, chosen automatically or in Setup), and the engine captures each program's own process tree
  (`ProcessCapture`, process loopback) into the channel strip through live, lock-free *app taps*
  (`engine.rs` `AppTapper`, up to 64, retired off the audio thread). A channel may instead own a
  *virtual output device* (a cable, later Audiora's own driver); then a loopback tap of that device
  feeds the strip. Either way nothing reaches the real speakers except through the master, and a
  channel can never listen to the master's device (that would loop forever). A route watcher keeps
  assigned programs on their channel as they start (the choice is stored by Windows per executable and
  re-checked every 1.5 s), remembers what the user had set before so letting go restores it, and
  releases every running program when the engine exits, so closing Audiora never leaves sound going to
  a device nobody listens to. Devices are chosen automatically (a device named for the channel, then a
  free virtual output; never one owned by a running Sonar/Voicemeeter; never the default output when it
  is virtual; never a Bluetooth hands-free microphone) or explicitly in Setup. An optional output
  receives the processed microphone for other programs to use through the cable's other end.
  Limits: programs that open a fixed device instead of the default move only after their audio is
  restarted; programs launched while Audiora is closed play to the virtual output (silent) until
  it runs again, the same dependency Sonar has.
- **Hardware control** (`audiora-device`): capability description + parameters + meters for any
  interface. Ships a labelled simulator; the real USB protocol is pending.
- **Licence** (`audiora-license`): offline Ed25519-signed files, optional lock to an R Interface
  serial; vendor tool issues them. Edition gating is a product decision not yet made.
- **Studio** (`audiora-svc/studio.rs`): multitrack projects in `Music\Audiora\Studio\<name>`. Tracks
  record any source — a channel's processed signal (`post/N`, e.g. the cleaned microphone), its raw
  input, or the master — into takes (one WAV, one stereo lane per armed track); tracks hold
  non-destructive clips (take, lane, start, offset, length). Playback mixes clips on a helper thread
  into a lock-free ring the audio thread adds to the master; recording plays the other tracks so
  parts can be overdubbed; waveform overviews are cached per take; export renders the mix to WAV.
- **Not built (needs the driver):** virtual *input* devices, so the processed microphone appears as a
  microphone by itself (today it leaves through a user-chosen cable).
- **Not built (large):** VST3 / CLAP hosting in a sandbox process.

### 4.3 Control plane
- Transports: loopback WebSocket (Origin allow-list + Host check, 16-client cap) for the app, and
  loopback TCP JSON-lines for tools. Both bind 127.0.0.1 only. Before release the TCP transport
  becomes a named pipe with an ACL; any local process can still talk to the engine today, which
  matches what Voicemeeter's remote API allows.
- Schema: JSON, parameter paths like `strip/3/eq/1/gain_db` — see `CONTROL-API.md`.
- Meters are pushed at up to 60 Hz after `subscribe`; everything else is request/response.
- Persistence: `%APPDATA%\Roujan\Audiora\` — `presets/`, `channel-presets/<channel>.json`,
  `channels.json` (devices), `routes.json` (program → channel), `app-profiles.json`, `license.json`.

### 4.4 Application (`app/`)
- **Tauri 2 + Svelte 5 (TypeScript)**. The shell (`app/src-tauri`) starts the engine if it is not
  running and asks it to shut down cleanly (recordings finalized, session saved) when the app exits
  — but only if the app started it.
- **Design system**: bundled Inter and JetBrains Mono (no network), five graphite surface steps,
  five switchable accent colours, one hue per channel, inline icon set. Hardware-style controls built
  for the job: rotary knobs (drag, wheel, keyboard, double-click reset, log scales), machined faders,
  canvas LED meters with peak hold, switches and segmented controls. Everything is keyboard
  operable and carries ARIA slider semantics.
- **Channel mixer**: Master at the far left with the live list of programs that make sound beneath it
  (found automatically); Game, Chat, Media, Aux and Mic columns to its right. Drag a program onto a
  column — or use the link menu — and it is routed there at once. Each column is deliberately plain:
  one volume slider in per cent (with the live level glowing inside its track), mute, and the apps
  it holds; everything else lives on the channel's page. Pointer-driven drag and drop (not the
  browser's) so it works inside the desktop shell and with touch.
- **Channel pages** (one per channel, like Sonar's): preset chooser with favourites and *Save as*,
  equalizer (Basic: Bass/Voice/Treble; Advanced: six bands) over a live spectrum, then one list of
  on/off rows — ClearCast noise cancellation (Chat, Mic), noise gate (Mic), volume boost, Smart Volume
  (Quiet/Balanced/Loud), compressor as a single *amount*, "hear yourself" (Mic) — followed by the
  apps in the channel with their own volumes and per-game profiles. Expert gate/compressor controls
  with transfer-curve graphs sit behind an *Advanced* disclosure.
  The channel pages are tabs in the header (Overview, Game, Chat, Media, Aux, Mic); the rail holds the
  areas: Mixer, **Studio** (track headers with arm/mute/solo/source/volume and live level, a canvas
  timeline with ruler, playhead, waveforms and a growing live region while recording, transport with
  keyboard shortcuts, split/move/delete clips, export), Interface and Setup.
  **Setup** assigns the virtual outputs, speakers, microphone and processed-microphone output, and
  shows audio health. The recorder and the audio-interface panel complete the rail.
  Layouts adapt down to the 980 x 620 minimum window.
- **Workflow**: click any read-out to type an exact value ("-6", "2.5k", "L30"); double-click a channel
  name to rename it (remembered per device); Ctrl+K opens a command palette (navigate, channel presets,
  mute channels, record, mute master, accent); Alt+1..9 switch views; toasts confirm actions and report errors.
  The EQ draws the live spectrum of its channel behind the curve; the master section shows LUFS.
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
continuous), live with two inputs and two outputs at 48 and 96 kHz, and in a 14.5-minute soak with
zero underruns, drops and device errors. One caveat from that soak: in one of two recorded 90 s takes
the master bus showed an unexplained -51 dBFS non-sine component while the app was being used
against the same engine; a repeat with nothing touching the engine was clean (two 60 s takes at the
expected -80.00 dBFS, perfectly continuous), so it was not reproduced.

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
| Svelte app + Tauri shell | Built; verified live against the engine in a browser and inside the Tauri window (dev and embedded-asset builds); installer not built |
| R Interface | Simulator only — real protocol unknown |
| Virtual audio driver | v1 source written (six endpoints, kernel mic cable); not built yet: WDK missing; test in a Hyper-V VM (`drivers/virtual-audio/README.md`) |
| ASIO driver | Designed — blocked: Steinberg licence decision |
| Per-app routing | Built on virtual cables (loopback + per-app output); verified live, including streams moving while they play. Own driver pending |
| VST3 / CLAP hosting, convolution, true-peak / LRA meters | Not started |
| Installer signing, updater, telemetry | Not started (needs certificates and hosting) |

## 11. Top risks

1. Kernel driver quality and signing lead time → start driver work early, on a dedicated machine.
2. R Interface firmware / USB descriptors unknown → see open questions in `R-INTERFACE.md`.
3. Long-run clock behaviour → soak tests (1 h+) still to run.
4. Conflicts with existing virtual audio software (Voicemeeter, Sonar) → detection and guidance in
   the installer.
5. Undocumented Windows per-app routing API → isolated in `audiora-hal/policy.rs` with both known
   interface ids and a read-back check; if a future Windows changes it the Setup page says routing is
   unavailable instead of failing silently.
