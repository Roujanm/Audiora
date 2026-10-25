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
| `info` | | `strips[]`, `buses[]` (index, kind, name, device_id, sample_rate, channels, period_frames, low_latency, exclusive, bit_depth), `rec_sources[]`, `recordings_dir`, `hardware` |
| `devices` | | `devices[]`: id, name, flow, default, sample_rate, channels |
| `apps` | | `apps[]`: instance_id, device_id, device_name, pid, name, volume (0–1), muted, peak, active, system_sounds — what the Windows volume mixer shows — plus `route` (channel the user assigned) and `playing_in` (channel whose device it plays into now) |
| `app_set` | `device`, `instance_id`, `volume` and/or `mute` | |
| `state` | | `params` (every path → value, meters included), `stats` |
| `get` / `set` | `path` (`value` for set) | `path`, `value` (after clamping) |
| `meters` | | `paths[]`, `values[]` (index-aligned) |
| `subscribe` *(WebSocket)* | `hz` 1–60 (default 30) | `hz`, `paths[]`, `band_hz[]` (96 spectrum band centres); then `{"event":"meters","v":[…]}` and `{"event":"analysis",…}` frames |
| `analysis` | | `m`, `s`, `i` (momentary, short-term, integrated loudness in LUFS; -120 = nothing measured yet) and `spectrum` (96 dBFS values, or null while the analyser is off) |
| `analysis_source` | `source`: `strip/N`, `bus/N`, an alias, or `null` | selects which signal feeds the spectrum analyser (off by default; the app switches it on only while the EQ view is open) |
| `loudness_reset` | | restarts the integrated-loudness measurement |
| `unsubscribe` *(WebSocket)* | | |
| `preset_list` | | `presets[]` |
| `preset_save` / `preset_load` / `preset_delete` | `name` | save: `saved`; load: `applied`, `skipped` |
| `rec_start` | `sources[]` (default `["mix"]`) | `path`, `sample_rate`, `channels`, `sources` |
| `rec_stop` | | `path`, `frames`, `seconds`, `dropped_frames` |
| `rec_list` | | `takes[]` (newest first, up to 40): name, path, bytes, modified_ms, channels, sample_rate, seconds |
| `rec_status` | | `recording`, and while recording `path`, `sources`, `seconds`, `dropped_frames`, `error` |
| `hw_info` | | `model`, `serial`, `firmware`, `simulated`, `inputs`, `outputs` |
| `hw_controls` | | `controls[]`: path, label, group, kind (`range`/`toggle`/`choice`), value |
| `hw_get` / `hw_set` | `path` (`value` for set) | `path`, `value` |
| `hw_meters` | | `meters` (path → linear peak) |
| `license_status` | | `state` (`dev-build`, `none`, `valid`, `invalid`), `edition`, … |
| `channels` | | `channels[]` (key, name, strip, kind `apps`/`mic`, device_id, device_name, auto, online, error), `master_out`, `mic_out`, `config` |
| `config_set` | any of `master_out`, `mic_in`, `mic_out` (id or null), `devices` (`{game: id\|null, …}`) | `restart: true` — saved to `channels.json`; applies after the engine restarts |
| `app_route` | `app` (executable name), `channel` (`game`/`chat`/`media`/`aux`/null) | `live` — sends the program to that channel's virtual output (null: back to where it was) |
| `routes` | | `routes`: program → channel |
| `cpreset_list` | `channel` | `factory[]`, `user[]`, `favorites[]` |
| `cpreset_save` / `cpreset_load` / `cpreset_delete` | `channel`, `name` | channel presets: EQ, boost, Smart Volume, noise suppression, gate, compressor of that channel's strip |
| `cpreset_favorite` | `channel`, `name`, `on` | |
| `app_profile_set` | `app`, `channel`, `preset` (or null) | loads that preset when the program starts making sound |
| `app_profiles` | | `profiles`: program → channel → preset |
| `shutdown` | | `stopping` — the engine finalizes recordings, saves the session, releases routed programs and exits |

Booleans may be sent as `true`/`false`; they are stored as 1/0. `hw_*` commands fail with
`no interface hardware connected` when none is attached.

## Parameter paths

Channel layout (default): strips 0–3 are Game, Chat, Media and Aux (loopback taps of their virtual
outputs), strip 4 is Mic, strip 5 the built-in tone strip; bus 0 is the master, an optional bus 1
carries only the processed microphone. `info` reports `channel_mode` and each strip's `channel`.
Flat layout (`--in` / `--no-input` / `--legacy`): every capture device is a strip, followed by the
tone strip; every output device is a bus. Bus 0 is the master.

| Path | Range | Notes |
|---|---|---|
| `strip/N/gain_db` | -120 … 24 | |
| `strip/N/balance` | -1 … 1 | 0 = unity on both channels |
| `strip/N/boost_db` | 0 … 12 | volume boost ahead of the chain |
| `strip/N/smart/enabled`, `target_db` (-40…-10) | | Smart Volume; `smart/gain_db` is a read-only meter |
| `strip/N/mute`, `strip/N/solo` | 0/1 | |
| `strip/N/bus_mask` | integer | bit *b* sends to bus *b*; capture strips start at 0 (unrouted) so a microphone never feeds back by default |
| `strip/N/denoise` | 0/1 | needs a 48 kHz master; +10 ms latency |
| `strip/N/gate/enabled`, `threshold_db` (-100…0), `attack_ms`, `release_ms`, `range_db` | | `gate/open` is a read-only meter |
| `strip/N/eq/B/freq`, `gain_db` (±24), `q` (0.1…18) | | B: 0 low shelf (sub bass), 1–4 peaks (bass, low mids, mids, upper mids), 5 high shelf (highs) |
| `strip/N/comp/enabled`, `threshold_db`, `ratio`, `attack_ms`, `release_ms`, `makeup_db`, `knee_db` | | `comp/gr_db` read-only |
| `strip/N/peak_l`, `peak_r` | read-only | linear peak of the last audio block |
| `bus/N/gain_db`, `bus/N/mute` | | |
| `bus/N/limiter/enabled`, `ceiling_db` (-30…0), `release_ms` | | `limiter/gr_db` read-only; on by default for bus 0 at -1 dB |
| `bus/N/peak_l`, `peak_r` | read-only | |

Strip chain: boost → denoise → gate → EQ → Smart Volume → compressor → fader/balance → buses.
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
audiora-svc [--in ID]... [--no-input] [--legacy] [--no-mic] [--out ID]... [--tone HZ] [--low-latency | --exclusive]
            [--port N] [--ws-port N] [--allow-origin URL]... [--rec-dir DIR]
            [--data-dir DIR] [--fresh] [--sim-r] [--secs S]
```

`--in` / `--out` repeat; `--in`, `--no-input` or `--legacy` select the flat layout, otherwise the channel
layout runs and `--out` (first one) overrides the automatically chosen master; `--no-mic` leaves the Mic
channel silent. `--sim-r` enables the R
Interface **simulator** (clearly labelled in `hw_info`). `--fresh` skips restoring the last session.
Device ids come from the `devices` command or `audiora devices`.

`--low-latency` opens shared streams at the smallest period the Windows audio engine offers.
`--exclusive` takes the devices exclusively in their native format (24/32-bit integer or float,
smallest period, about 3 ms on typical onboard audio) — lowest latency and bit-perfect, but no other
application can use those devices meanwhile and Windows system sounds on them are silenced. If the
device refuses every exclusive format the engine does not start and says why.

`stats` explains audio health: `underruns` (a device read from an empty ring), `drift_drops` (a ring
overfilled beyond what the resampler can trim), `capture_drops` (a ring was full), `device_errors`
(a device stream died — unplugged, or another application took it exclusively). After a device
error the engine keeps running with silence on that path and reopens the device by itself once it is
back with the same rate and channel count. A few underruns at start-up are normal.
