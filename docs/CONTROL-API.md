# Audiora Control API (v1)

Transport: loopback TCP `127.0.0.1:7878` (default), UTF-8, **one JSON object per line** in each
direction. This is the same API the Svelte app will use; a named pipe with an ACL replaces TCP
before release (see ARCHITECTURE.md §7).

Every reply has `"ok": true|false` and echoes the request's `"id"`. Errors add `"error"`.

| Command | Request | Reply (besides ok/id) |
|---|---|---|
| `ping` | `{"cmd":"ping"}` | `api` |
| `info` | `{"cmd":"info"}` | `output`, `input`, `strips`, `buses` |
| `devices` | `{"cmd":"devices"}` | `devices[]`: id, name, flow, default, sample_rate, channels |
| `state` | `{"cmd":"state"}` | `params` (all paths → value, incl. meters), `stats` |
| `get` | `{"cmd":"get","path":"strip/0/gain_db"}` | `path`, `value` |
| `set` | `{"cmd":"set","path":"…","value":-6}` | `path`, `value` (after clamping) |

| `rec_start` | `{"cmd":"rec_start","sources":["mix","input"]}` | `path`, `sample_rate`, `channels`, `sources` |
| `rec_stop` | `{"cmd":"rec_stop"}` | `path`, `frames`, `seconds`, `dropped_frames` |
| `rec_status` | `{"cmd":"rec_status"}` | `recording`, and while recording: `path`, `frames`, `seconds`, `dropped_frames`, `error` |

Booleans may be sent as `true/false`; they are stored as 1/0.

## Recording

Takes are 32-bit float WAV, written to `%USERPROFILE%\Music\Audiora\Recordings` (override with
`--rec-dir`) as `take-YYYYMMDD-HHMMSS.wav`; existing files are never overwritten.

`sources` picks what to record. Every source is one stereo pair, so channels = 2 × sources, in the
order requested (default `["mix"]`):

| Source | Signal |
|---|---|
| `input` | capture device, raw (before gain/EQ) — record the untouched take |
| `tone` | built-in oscillator, raw |
| `mix` | bus 0, after the mixer |

Files with more than 2 channels use `WAVE_FORMAT_EXTENSIBLE`; anything over 4 GB is written as RF64.
The header is re-patched about once per second of audio, so a crash or power loss keeps everything
up to the last second. Stopping the service finalizes an active take. `dropped_frames` counts audio
the disk could not keep up with (whole blocks are dropped, so channels never shift).

## Parameter paths

| Path | Range | Notes |
|---|---|---|
| `strip/N/gain_db` | -120 … 24 | |
| `strip/N/balance` | -1 … 1 | 0 = unity both channels |
| `strip/N/mute`, `strip/N/solo` | 0/1 | |
| `strip/N/bus_mask` | integer | bit *b* = feeds bus *b* |
| `strip/N/eq/B/freq` | 20 … 20000 | B = 0 low shelf, 1–2 peak, 3 high shelf |
| `strip/N/eq/B/gain_db` | -24 … 24 | |
| `strip/N/eq/B/q` | 0.1 … 18 | |
| `bus/N/gain_db`, `bus/N/mute` | | |
| `strip/N/peak_l|peak_r`, `bus/N/peak_l|peak_r` | read-only | linear peak of the last audio block |

Current service layout: strip 0 = capture device, strip 1 = built-in tone generator, bus 0 = output.

## Try it

```powershell
cargo run -p audiora-svc -- --no-input --tone 440 --secs 10
```

Then connect any TCP client to port 7878 and send lines such as
`{"id":1,"cmd":"set","path":"strip/1/gain_db","value":-36}`.

`stats` explains audio health: `underruns` (render wanted audio before capture delivered it),
`drift_drops` (capture ran ahead and old audio was dropped), `capture_drops` (ring full).
