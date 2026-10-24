# Audiora Roadmap

Legend: ✅ done and verified here · 🟡 partly done · ⛔ blocked on something outside the code · ⬜ not started

| Phase | Scope | State |
|---|---|---|
| **0 Foundation** | Workspace, DSP, mixer, ring, WASAPI, CLI | ✅ |
| **1 Engine** | Multi-device graph, drift-tracking resampler, low-latency WASAPI, control API, presets, WebSocket | ✅ built, incl. WASAPI exclusive and device-loss recovery · 🟡 15-min soak run, 1 h+ not run · ⬜ named-pipe transport with ACL |
| **2 Recorder** | Multichannel 32-bit float WAV/RF64, crash-safe, engine integration | ✅ · ⬜ punch-in/out, take management, input monitoring |
| **3 App** | Tauri 2 + Svelte 5: Play and Studio skins | ✅ UI verified live · 🟡 Tauri shell compiles; installer build and window run not exercised · ⬜ spectrum / LUFS displays |
| **4 Virtual audio driver** | SysVAD-derived kernel driver, per-app routing | ⛔ designed; needs WDK + a test machine |
| **5 ASIO + R Interface** | ASIO driver, R backend, hardware control | ⛔ ASIO needs the Steinberg licence decision; R needs hardware documentation · ✅ simulator + control layer |
| **6 Processing** | Gate, compressor, limiter, denoiser | ✅ · ⬜ de-esser, reverb/convolution, VST3/CLAP hosting, LUFS meters |
| **7 Release** | Signing, installer, updater, licensing | ✅ licence format + tool, portable zip, per-user install script · ⛔ code-signing certificate · ⬜ updater, telemetry (opt-in) |

## What unblocks what

| Blocker | Who | Unblocks |
|---|---|---|
| Install the Windows Driver Kit; provide a VM or spare PC | Roujan Digital Agency | Phase 4 |
| Sign Steinberg's ASIO licence agreement | Roujan Digital Agency (legal) | ASIO driver |
| R Interface USB descriptors, control protocol, serial format (`R-INTERFACE.md`) | Hardware / firmware team | Real R backend, licence device-lock, ASIO tuning |
| Code-signing certificate; Hardware Dashboard account | Roujan Digital Agency | Signed installer and drivers |
| Decide edition limits (what is Play vs Studio) | Product | Licence gating |
