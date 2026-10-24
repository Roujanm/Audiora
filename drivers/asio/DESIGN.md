# Audiora ASIO driver — design

Status: **design only**, blocked on a licensing decision (below).

## Why

DAWs (Cubase, Reaper, FL Studio, Ableton, Studio One) expect an ASIO driver for low-latency
recording. Windows' own USB Audio 2.0 driver does not provide ASIO. Audiora's ASIO driver is what
turns the R Interface — and any other device — into a studio interface.

## Licensing — decision needed from Roujan Digital Agency

ASIO is a Steinberg trademark and SDK. Steinberg offers the SDK under GPLv3 or a proprietary
licence agreement. A closed-source product needs the proprietary agreement (free of charge, but it
has to be signed, and it governs use of the "ASIO" name). **No ASIO code has been written** so that
no GPL or licence obligation attaches to the repository before that decision. Everything the driver
needs on the Audiora side already exists and is independent of the ASIO interface definition.

## Architecture

```
 DAW process                                        audiora-svc (user session)
 ┌───────────────────────────┐                      ┌──────────────────────────┐
 │ AudioraASIO.dll (COM,     │  device lease (API)  │ owns devices by default  │
 │ in-proc, IASIO)           │ ───────────────────► │ releases R Interface on  │
 │  bufferSwitch thread (MMCSS│                     │ lease, takes it back     │
 │  "Pro Audio")             │                      └──────────────────────────┘
 │  ▼                        │
 │ R Interface backend       │── USB streaming ──► R Interface
 └───────────────────────────┘
```

* **Tier 1 (R Interface, lowest latency):** the DLL opens the device itself through the
  `audiora-hal` R Interface backend (WASAPI exclusive until a native USB streaming driver exists).
  Only one process can own the stream, so the engine *leases* it: the DLL asks the engine through
  the control API (`device_lease`), the engine closes its streams, and takes the device back when
  the DAW stops. The user sees "R Interface is in use by Cubase" in the UI.
* **Tier 2 (any other device):** the DLL either leases a WASAPI exclusive device the same way, or
  connects to the engine's shared-memory rings (like the virtual-driver endpoints) for shared mode.
* **ASIO surface:** `init`, `getChannels`, `getBufferSize` (min/preferred/max/granularity),
  `canSampleRate`/`setSampleRate`, `createBuffers` (double buffers), `start`/`stop`,
  `getLatencies`, `getChannelInfo` (`ASIOSTFloat32LSB` or `Int32LSB` per hardware),
  `getSamplePosition`, `outputReady`, `controlPanel` (opens the Audiora app), `future`.
* **Real-time rules:** the buffer-switch thread never allocates or locks; format conversion is
  table-free SIMD; latency numbers reported to the host are measured, not guessed.
* **Registration:** `HKLM\SOFTWARE\ASIO\Audiora` (+ WOW6432Node for 32-bit hosts) with `CLSID` and
  description; the CLSID's `InprocServer32` points at the DLL. Done by the installer.
* **Bitness:** ship 64-bit; add 32-bit only if customers still use 32-bit hosts.

## Language

Rust (`cdylib`) with a thin hand-written COM vtable layer. The audio path reuses `audiora-hal` and
`audiora-core::ring`; no C++ is needed.

## Test plan

* ASIO host test: Steinberg's ASIO test host / Reaper / Cubase LE — open, change buffer size and
  sample rate, start/stop 1000 times, run 8 h.
* Latency: loopback cable measurement; report round-trip at 64/128/256 frames, 48 and 96 kHz.
* Hot-unplug during streaming: host must receive `kAsioResetRequest`, not hang.
