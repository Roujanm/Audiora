# R Audio Interface — Integration Contract

The R interface is Roujan Digital Agency's hardware. Audiora is its official driver and control
software. These are the facts Audiora needs from the hardware/firmware side. Items marked
**OPEN** are not decided yet and block the matching Audiora work.

## Compatibility tiers

| Tier | Devices | Audiora provides |
|---|---|---|
| 1 | R Interface | Own ASIO driver, hardware control, direct monitoring, firmware update, device-locked features |
| 2 | Any USB-class-compliant or WASAPI device | Mixer, recording, ASIO wrapper over WASAPI |

## Open questions (hardware / firmware)

1. **OPEN — USB class**: UAC2 class-compliant, or vendor-specific? Class-compliant devices work
   with the inbox Windows driver immediately; Audiora then adds ASIO + control.
2. **OPEN — I/O**: input/output counts, mic preamps, line/instrument, phantom power (+48 V),
   headphone outs, MIDI, S/PDIF/ADAT.
3. **OPEN — formats**: sample rates (44.1–192 kHz?), bit depth (24/32), clock source options.
4. **OPEN — control channel**: how gain, pad, phantom, monitor mix, mute are set — USB vendor
   control requests, HID, or MIDI SysEx? Needs a documented message list.
5. **OPEN — direct monitoring**: hardware mixer? Zero-latency monitor mix controllable from Audiora?
6. **OPEN — identity**: USB VID/PID, serial number format (used for device-locked licence and
   "recognise my interface" onboarding).
7. **OPEN — firmware update**: DFU mechanism and signing.
8. **OPEN — latency goal**: target round-trip at 48/96 kHz and minimum buffer size.

## What already exists on the Audiora side

* `audiora-device` defines what the rest of the product needs from *any* interface: a capability
  description (controls with ranges, toggles and choices), parameters by path, and meters.
* `SimulatedR` implements it as a software stand-in (2 combo inputs, main + headphone outputs, direct
  monitor mix, clock source). It is enabled with `--sim-r`, reports `simulated: true`, and the app
  shows a "SIMULATOR — not real hardware" badge. It enforces the safety rule real hardware should
  too: +48 V only on a mic source, and it drops phantom power when the source changes.
* `audiora-license` can lock a licence to the interface's serial number, so a purchase of the R
  Interface can include the Studio edition.
* The Interface tab of the app is built from the capability description, so a real backend appears
  there without UI changes.

When the real protocol is known, a new type implements `HardwareInterface`; callers do not change.

## What Audiora will do once these are known

- `audiora-hal::r_interface` backend: enumerate by VID/PID, expose channels with real names
  (Mic 1, Line 2, Loopback…), report true latency, own the master clock.
- ASIO driver bound to the device's streaming endpoint.
- Control panel in the Studio skin: gain, phantom, pad, monitor mix, clock, meters.
- Zero-config recording: plug in → select inputs → record 32-bit float WAV.
- Installer detects the device, installs/updates driver and firmware in one flow.
