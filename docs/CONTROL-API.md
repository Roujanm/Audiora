# Audiora Control API (v1)

Two loopback transports, same commands:

| Transport | Address | Framing | For |
|---|---|---|---|
| WebSocket | `ws://127.0.0.1:7879` | one JSON message per text frame | the app (Origin-checked) |
| TCP | `127.0.0.1:7878` | one JSON object per line | scripts and tools |

The WebSocket handshake is refused (HTTP 403) unless the `Host` header is a loopback name on that
port and the `Origin` header, if present, is one of `tauri://localhost`, `http(s)://tauri.localhost`,
`http://localhost:5173`, `http://127.0.0.1:5173` or an `--allow-origin` given at start. Requests
without an `Origin` (scripts, native clients) are accepted.

Every reply has `"ok": true|false` and echoes the request's `"id"`. Errors add `"error"`.

## Commands

| Command | Request fields | Reply fields |
|---|---|---|
| `ping` | | `api` |
| `info` | | `strips[]`, `buses[]` (index, kind, name, device_id, sample_rate, channels, period_frames, low_latency), `rec_sources[]`, `recordings_dir`, `hardware` |
| `devices` | | `devices[]`: id, name, flow, default, sample_rate, channels |
| `state` | | `params` (every path → value, meters included), `stats` |
| `get` / `set` | `path` (`value` for set) | `path`, `value` (after clamping) |
| `meters` | | `paths[]`, `values[]` (index-aligned) |
| `subscribe` *(WebSocket)* | `hz` 1–60 (default 30) | `hz`, `paths[]`; then `{"event":"meters","v":[…]}` frames in that order |
| `unsubscribe` *(WebSocket)* | | |
| `preset_list` | | `presets[]` |
| `preset_save` / `preset_load` / `preset_delete` | `name` | save: `saved`; load: `applied`, `skipped` |
| `rec_start` | `sources[]` (default `["mix"]`) | `path`, `sample_rate`, `channels`, `sources` |
| `rec_stop` | | `path`, `frames`, `seconds`, `dropped_frames` |
| `rec_status` | | `recording`, and while recording `path`, `sources`, `seconds`, `dropped_frames`, `error` |
| `hw_info` | | `model`, `serial`, `firmware`, `simulated`, `inputs`, `outputs` |
| `hw_controls` | | `controls[]`: path, label, group, kind (`range`/`toggle`/`choice`), value |
| `hw_get` / `hw_set` | `path` (`value` for set) | `path`, `value` |
| `hw_meters` | | `meters` (path → linear peak) |
| `license_status` | | `state` (`dev-build`, `none`, `valid`, `invalid`), `edition`, … |
| `shutdown` | | `stopping` — the engine finalizes recordings, saves the session and exits |

Booleans may be sent as `true`/`false`; they are stored as 1/0. `hw_*` commands fail with
`no interface hardware connected` when none is attached.

## Parameter paths

Layout: every capture device is a strip (`input`), followed by one built-in tone strip; every output
device is a bus. Bus 0 is the master. `info` lists them with their device names.

| Path | Range | Notes |
|---|---|---|
| `strip/N/gain_db` | -120 … 24 | |
| `strip/N/balance` | -1 … 1 | 0 = unity on both channels |
| `strip/N/mute`, `strip/N/solo` | 0/1 | |
| `strip/N/bus_mask` | integer | bit *b* sends to bus *b*; capture strips start at 0 (unrouted) so a microphone never feeds back by default |
| `strip/N/denoise` | 0/1 | needs a 48 kHz master; +10 ms latency |
| `strip/N/gate/enabled`, `threshold_db` (-100…0), `attack_ms`, `release_ms`, `range_db` | | `gate/open` is a read-only meter |
| `strip/N/eq/B/freq`, `gain_db` (±24), `q` (0.1…18) | | B: 0 low shelf, 1–2 peak, 3 high shelf |
| `strip/N/comp/enabled`, `threshold_db`, `ratio`, `attack_ms`, `release_ms`, `makeup_db`, `knee_db` | | `comp/gr_db` read-only |
| `strip/N/peak_l`, `peak_r` | read-only | linear peak of the last audio block |
| `bus/N/gain_db`, `bus/N/mute` | | |
| `bus/N/limiter/enabled`, `ceiling_db` (-30…0), `release_ms` | | `limiter/gr_db` read-only; on by default for bus 0 at -1 dB |
| `bus/N/peak_l`, `peak_r` | read-only | |

Strip chain: denoise → gate → EQ → compressor → fader/balance → buses.
Bus chain: sum → fader → limiter.

## Recording

Takes are 32-bit float WAV, written to `%USERPROFILE%\Music\Audiora\Recordings` (`--rec-dir`
overrides) as `take-YYYYMMDD-HHMMSS.wav`; existing files are never overwritten.

`sources` picks what to record; each source is one stereo track and channels = 2 × sources, in the
order requested.

| Source | Signal |
|---|---|
| `strip/N` | strip N raw, before gain/EQ/dynamics (record the untouched take) |
| `bus/N` | bus N after the mixer — what you hear |
| aliases | `input` = `strip/0` (when there is an input), `tone` = the tone strip, `mix` = `bus/0` |

Files with more than 2 channels use `WAVE_FORMAT_EXTENSIBLE`; anything over 4 GB is written as RF64.
The header is re-patched about once per second of audio, so a crash keeps everything up to the last
second. Stopping the engine finalizes an active take. `dropped_frames` counts audio the disk could not
keep up with (whole blocks are dropped, so channels never shift).

## Starting the engine

```text
audiora-svc [--in ID]... [--no-input] [--out ID]... [--tone HZ] [--low-latency]
            [--port N] [--ws-port N] [--allow-origin URL]... [--rec-dir DIR]
            [--data-dir DIR] [--fresh] [--sim-r] [--secs S]
```

`--in` / `--out` repeat; with none given the system defaults are used. `--sim-r` enables the R
Interface **simulator** (clearly labelled in `hw_info`). `--fresh` skips restoring the last session.
Device ids come from the `devices` command or `audiora devices`.

`stats` explains audio health: `underruns` (a device read from an empty ring), `drift_drops` (a ring
overfilled beyond what the resampler can trim), `capture_drops` (a ring was full). A few at start-up
are normal.
