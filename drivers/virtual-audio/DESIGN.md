# Audiora Virtual Audio — driver design

Status: **v1 source written** (`Source/`, see [README.md](README.md)); not yet built: needs the Windows
Driver Kit and a test VM. v1 differs from the full design below in one place: instead of the control
interface and shared-memory rings, the engine records the four outputs with WASAPI loopback (as it
does for any output today) and the microphone is a kernel *cable* (Mic Feed output → Microphone
input). That keeps the kernel side to a ring buffer and no IOCTLs; the rings remain the plan for
lower latency.

## Goal

Give every application a set of named playback devices (Game, Chat, Media, Aux) and a virtual
microphone, so the Audiora engine can mix them per channel. Everything else (EQ, dynamics, routing, recording) already lives in the engine and
works today on real devices; the driver's only job is to *deliver and accept audio*.

## Endpoints

| Endpoint | Direction | Format offered | Notes |
|---|---|---|---|
| Audiora Game | render | 44.1–192 kHz, 16/24/32f, 2 ch and 7.1 | 7.1 so games keep surround |
| Audiora Chat | render | 44.1–96 kHz, 16/24/32f, 2 ch | |
| Audiora Media | render | 44.1–192 kHz, 2 ch | |
| Audiora Aux | render | 44.1–96 kHz, 2 ch | |
| Audiora Microphone | capture | 44.1–96 kHz, 2 ch | fed by the engine's processed mic bus |

Names are fixed in the INF; the user never has to pick "Audiora Driver 3".

## Structure

Derived from Microsoft's SysVAD sample (MS-PL licence, which allows commercial derivative drivers):
one WDM adapter (`ROOT\Audiora` root-enumerated device), one WaveRT miniport + topology miniport per
endpoint. SysVAD already demonstrates multiple endpoints, WaveRT, and the property handlers Windows
expects. Remove from the sample: APO samples, Bluetooth/HFP endpoints, speech/keyword detector, the
tablet-specific topology.

```
 app ─WASAPI─► audiodg (Windows audio engine) ─► WaveRT cyclic buffer (kernel, per endpoint)
                                                   │ driver copies each period
                                                   ▼
                              shared-memory ring per endpoint  (one kernel section)
                                                   │ mapped read-only
                                                   ▼
                        audiora-svc: `DriftResampler` per ring ─► Mixer strip
```

* **Control interface.** A second device interface (`GUID_AUDIORA_CONTROL`) accepts IOCTLs from the
  engine only (secured with an SDDL that grants the current interactive user plus SYSTEM):
  `GET_LAYOUT` (endpoints, rings, formats), `MAP_RING(endpoint)` (returns a section handle valid in
  the caller), `SET_EVENT(endpoint, handle)` (driver signals when a period was delivered / consumed).
* **Ring layout.** Header (magic, version, sample rate, channels, format, write/read counters as
  64-bit monotonic values) followed by the sample area; same wait-free single-producer /
  single-consumer protocol as `audiora_core::ring`, so the user-mode side reuses the existing,
  tested code.
* **Clocking.** The driver advances each WaveRT position from a kernel timer on the system clock.
  The engine bridges that clock to the output device with `DriftResampler` — the same component
  already proven for two physical devices. No special case is needed for virtual clocks.
* **Capture endpoint** works in reverse: the engine writes the processed microphone bus into a ring
  and the driver copies it into the capture WaveRT buffer.
* **No DSP in the kernel.** Not even gain. A crash in the driver blue-screens the machine; a crash
  in the engine does not.

## Per-application routing

Windows 10/11 lets users assign an output device per application (Settings → Sound → Volume mixer).
The engine's UI will set that preference for the user's applications. There is no documented API; the
well-known mechanism is the `AudioPolicyConfig` activation factory, which changes between Windows
builds. Plan: wrap it behind one module with a version
guard, test on each Windows feature update, and fall back to telling the user to use Windows' own
setting.

## Install / uninstall

* `Audiora.inf` + catalog; installed with `pnputil /add-driver Audiora.inf /install` and a
  root-enumerated device created by a small helper (`devcon install` equivalent via SetupAPI).
* Installer detects other virtual audio software and warns if it holds the same default device.
* Uninstall removes the device first, then the driver package; endpoints disappear cleanly.

## Verification before any release

1. Driver Verifier (standard + I/O verification) clean for a 24 h soak with 5 streams.
2. Windows Hardware Lab Kit audio tests (WaveRT, HLK "Audio Fundamental") — required for the
   Hardware Dashboard submission regardless of signing route.
3. Hot-plug of other devices, sleep/resume, fast user switching, default-device change while
   streaming, sample-rate change in the Windows Sound control panel.
4. Glitch test: loopback a sine through Game → engine → output for 1 h, check bit-exactness /
   continuity with the recorder.

## Signing and distribution

Kernel-mode drivers must be signed through the Microsoft Hardware Dashboard (attestation or HLK).
Which certificate type the dashboard account needs (EV vs standard code-signing) must be read from
Microsoft's current requirements page before buying — sources disagreed when this was researched.
Development builds use test-signing on the dedicated test machine only.

## Blockers on the current machine

* The Windows Driver Kit is not installed (Visual Studio 2022 with the C++ workload is).
* The only machine available also runs other virtual audio drivers; testing a new audio driver there
  risks blue screens and endpoint conflicts. Use a VM or spare PC.
