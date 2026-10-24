# Audiora

Professional Windows audio platform by **Roujan Digital Agency**: a per-app mixer for gaming and
daily use, a low-latency recording studio, and the official software for the **R audio
interface**.

Proprietary — all rights reserved.

## Status

Phase 0 (foundation) is done: DSP, mixer engine, lock-free ring buffer, WASAPI backend, CLI.
See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md), [docs/ROADMAP.md](docs/ROADMAP.md) and
[docs/R-INTERFACE.md](docs/R-INTERFACE.md).

## Try it

```powershell
cargo test
cargo run -p audiora-cli -- devices
cargo run -p audiora-cli -- tone --gain -30 --secs 1      # default output; start quiet
```

## Requirements

Rust (stable, MSVC), Windows 10/11. Kernel driver work additionally needs Visual Studio 2022
with the Windows Driver Kit.
