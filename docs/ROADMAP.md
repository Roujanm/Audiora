# Audiora Roadmap

| Phase | Goal | Exit criteria |
|---|---|---|
| **0 Foundation** ✅ | Workspace, DSP, mixer, ring buffer, WASAPI render, CLI | `cargo test` green; `audiora tone` plays through the mixer |
| **1 Engine** 🟡 | `audiora-svc`: WASAPI capture + render, control API (done); multi-device graph, drift-tracking SRC, Windows service wrapper, 1 h soak test (open) | Mic → mixer → headphones live, no dropouts for 1 h |
| **2 Recorder** 🟡 | `audiora-rec`: multitrack 32-bit float WAV, RF64, crash-safe headers, service integration (done); punch-in/out, take management, input-monitoring, per-device tracks (open) | 8-track 1 h take, bit-exact vs. input |
| **3 App** | Tauri 2 + Svelte: Play skin first, then Studio skin | Mixer UI drives engine, 60 fps meters |
| **4 Virtual audio driver** | SysVAD-derived kernel driver (Game/Chat/Media/Aux/Mic), per-app routing | Apps routed per channel; Driver Verifier clean |
| **5 ASIO + R Interface** | ASIO driver, R backend, hardware control | DAW records from R at target latency |
| **6 Plug-ins & DSP** | VST3/CLAP host, compressor/gate/limiter, noise suppression | Plug-in crash doesn't interrupt audio |
| **7 Release** | Signing, installer, updater, licensing, telemetry opt-in | Clean install on fresh Windows 10/11 |

Dependencies: phase 4 needs Visual Studio + WDK installed and a dedicated test machine;
phase 5 needs the answers in `R-INTERFACE.md`.
