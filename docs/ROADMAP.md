# Audiora Roadmap

Legend: ✅ done and verified here · 🟡 partly done · ⛔ blocked on something outside the code · ⬜ not started

| Phase | Scope | State |
|---|---|---|
| **0 Foundation** | Workspace, DSP, mixer, ring, WASAPI, CLI | ✅ |
| **1 Engine** | Multi-device graph, drift-tracking resampler, low-latency WASAPI, control API, presets, WebSocket | ✅ built, incl. WASAPI exclusive and device-loss recovery · ✅ 14.5-min soak (mic + tone, 48 kHz master + 96 kHz second bus, recording twice): 87,242 callbacks, 0 underruns / drops / device errors · 🟡 1 h+ not run · ⬜ named-pipe transport with ACL |
| **2 Recorder** | Multichannel 32-bit float WAV/RF64, crash-safe, engine integration | ✅ · ⬜ punch-in/out, take management, input monitoring |
| **3 App** | Tauri 2 + Svelte 5: one professional interface | ✅ Sonar-style channel mixer, channel pages, Setup, recorder, interface panel; verified live in a browser and in the Tauri window · 🟡 NSIS installer not built · ✅ live spectrum in the EQ, LUFS on the master, command palette, typed values |
| **4 Per-app routing** | Drag-and-drop routing of programs into Game/Chat/Media/Aux | ✅ via Windows per-app output + loopback on third-party virtual cables (live moves verified, routes released on exit) · ⛔ own SysVAD-derived driver (real virtual microphone, no cable install) needs WDK + a test machine |
| **5 ASIO + R Interface** | ASIO driver, R backend, hardware control | ⛔ ASIO needs the Steinberg licence decision; R needs hardware documentation · ✅ simulator + control layer |
| **6 Processing** | Gate, compressor, limiter, denoiser | ✅ · ✅ 6-band EQ, volume boost, Smart Volume, channel presets and per-game profiles · ✅ LUFS loudness meter and spectrum analyser · ⬜ spatial audio, de-esser, reverb/convolution, VST3/CLAP hosting, true-peak/LRA |
| **7 Release** | Signing, installer, updater, licensing | ✅ licence format + tool, portable zip, per-user install script · ⛔ code-signing certificate · ⬜ updater, telemetry (opt-in) |

## What unblocks what

| Blocker | Who | Unblocks |
|---|---|---|
| Install the Windows Driver Kit; provide a VM or spare PC | Roujan Digital Agency | Phase 4 own driver (virtual microphone, cable-free setup) |
| Sign Steinberg's ASIO licence agreement | Roujan Digital Agency (legal) | ASIO driver |
| R Interface USB descriptors, control protocol, serial format (`R-INTERFACE.md`) | Hardware / firmware team | Real R backend, licence device-lock, ASIO tuning |
| Code-signing certificate; Hardware Dashboard account | Roujan Digital Agency | Signed installer and drivers |
| Decide edition limits (what each edition unlocks) | Product | Licence gating |
