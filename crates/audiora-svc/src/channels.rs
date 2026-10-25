//! The Sonar-style channel model: Game, Chat, Media and Aux are application mixes, Mic is the
//! microphone. Each application mix is a virtual output device; Windows is told (per
//! application) to play into it and the engine listens to it with a loopback tap, so the mix
//! reaches the mixer without ever touching the real speakers.
//!
//! Everything here is persisted in the data directory: the device assignment (`channels.json`),
//! which application goes to which channel (`routes.json`) and per-game preset profiles
//! (`app-profiles.json`).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use audiora_core::MixerParams;
use audiora_hal::{DeviceInfo, Flow};
use serde_json::{json, Map, Value};

use crate::cpresets::CPresets;

/// Channel key and display name, in mixer order. The first four are application mixes.
pub const CHANNELS: [(&str, &str); 5] =
    [("game", "Game"), ("chat", "Chat"), ("media", "Media"), ("aux", "Aux"), ("mic", "Mic")];
pub const APP_CHANNELS: usize = 4;
pub const MIC: usize = 4;

pub fn channel_index(key: &str) -> Option<usize> {
    CHANNELS.iter().position(|(k, _)| *k == key)
}

/// User-chosen devices; `None` = automatic.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Config {
    pub master_out: Option<String>,
    pub mic_in: Option<String>,
    /// Optional render device that receives the processed microphone (a virtual cable input),
    /// so other programs can pick the cable's output as their microphone.
    pub mic_out: Option<String>,
    pub devices: [Option<String>; APP_CHANNELS],
}

fn opt(v: &Value) -> Option<String> {
    v.as_str().filter(|s| !s.is_empty()).map(str::to_owned)
}

impl Config {
    pub fn load(dir: &Path) -> Self {
        let v: Value = fs::read_to_string(dir.join("channels.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or(Value::Null);
        let mut c = Config { master_out: opt(&v["master_out"]), mic_in: opt(&v["mic_in"]), mic_out: opt(&v["mic_out"]), ..Default::default() };
        for (i, (key, _)) in CHANNELS[..APP_CHANNELS].iter().enumerate() {
            c.devices[i] = opt(&v["devices"][*key]);
        }
        c
    }

    pub fn to_json(&self) -> Value {
        let mut devices = Map::new();
        for (i, (key, _)) in CHANNELS[..APP_CHANNELS].iter().enumerate() {
            devices.insert((*key).into(), json!(self.devices[i]));
        }
        json!({ "master_out": self.master_out, "mic_in": self.mic_in, "mic_out": self.mic_out, "devices": devices })
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        write_json(&dir.join("channels.json"), &self.to_json())
    }
}

fn write_json(path: &Path, v: &Value) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(v).unwrap()).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

const VIRTUAL_HINTS: [&str; 8] = ["sonar", "voicemeeter", "vb-audio", "cable", "virtual", "audiora", "steelseries", "hi-fi"];

pub fn is_virtual(name: &str) -> bool {
    let n = name.to_lowercase();
    VIRTUAL_HINTS.iter().any(|h| n.contains(h))
}

/// Lower is a better automatic choice for where the user listens: headphones and speakers before
/// the analogue jack, before HDMI/digital outputs that usually have nothing attached.
fn listening_rank(name: &str) -> u8 {
    let n = name.to_lowercase();
    if ["headphone", "headset", "speaker", "earphone", "earbud"].iter().any(|h| n.contains(h)) {
        0
    } else if ["hdmi", "digital", "spdif", "s/pdif", "monitor", "display", "high definition audio"].iter().any(|h| n.contains(h)) {
        2
    } else {
        1
    }
}

/// Another mixer that is running owns its own virtual devices; using them would play every
/// sound twice (once through that mixer, once through Audiora).
fn owned_by_running_mixer(name: &str, running: &[String]) -> bool {
    let n = name.to_lowercase();
    let any = |frags: &[&str]| running.iter().any(|p| frags.iter().any(|f| p.contains(f)));
    ((n.contains("sonar") || n.contains("steelseries")) && any(&["steelseriessonar", "steelseriesgg"]))
        || ((n.contains("voicemeeter") || n.contains("vb-audio voicemeeter")) && any(&["voicemeeter"]))
}

/// What the engine will actually use.
#[derive(Clone, Debug, Default)]
pub struct Resolved {
    pub master_out: Option<String>,
    pub mic_in: Option<String>,
    pub mic_out: Option<String>,
    pub devices: [Option<String>; APP_CHANNELS],
    /// True where a channel device was picked automatically.
    pub auto: [bool; APP_CHANNELS],
}

pub fn resolve(cfg: &Config, devs: &[DeviceInfo], running: &[String]) -> Resolved {
    let exists = |id: &Option<String>, flow: Flow| id.as_ref().filter(|i| devs.iter().any(|d| &d.id == *i && d.flow == flow)).cloned();
    let mut r = Resolved {
        master_out: exists(&cfg.master_out, Flow::Render),
        mic_in: exists(&cfg.mic_in, Flow::Capture),
        mic_out: exists(&cfg.mic_out, Flow::Render),
        ..Default::default()
    };
    // Master: the system default unless it is virtual (then it would be a loop), else the most
    // plausible real output.
    if r.master_out.is_none() {
        r.master_out = devs
            .iter()
            .find(|d| d.flow == Flow::Render && d.is_default && !is_virtual(&d.name))
            .or_else(|| {
                devs.iter().filter(|d| d.flow == Flow::Render && !is_virtual(&d.name)).min_by_key(|d| listening_rank(&d.name))
            })
            .map(|d| d.id.clone());
    }
    // Microphone: never a Bluetooth hands-free microphone (8 kHz): opening it would drop the
    // headset's playback quality to telephone grade.
    if r.mic_in.is_none() {
        let usable = |d: &&DeviceInfo| d.flow == Flow::Capture && !is_virtual(&d.name) && d.sample_rate >= 16_000;
        r.mic_in = devs.iter().filter(usable).find(|d| d.is_default).or_else(|| devs.iter().find(usable)).map(|d| d.id.clone());
    }
    let mut used: HashSet<String> = HashSet::new();
    used.extend(r.master_out.clone());
    used.extend(r.mic_out.clone());
    for i in 0..APP_CHANNELS {
        r.devices[i] = exists(&cfg.devices[i], Flow::Render);
        used.extend(r.devices[i].clone());
    }
    // Automatic picks: a device literally named for the channel first, then any free virtual
    // output nobody else is using.
    let mut pool: Vec<&DeviceInfo> = devs
        .iter()
        .filter(|d| {
            let n = d.name.to_lowercase();
            d.flow == Flow::Render
                && is_virtual(&d.name)
                && !owned_by_running_mixer(&d.name, running)
                && !n.contains("microphone")
                && !n.contains("mic")
        })
        .collect();
    pool.sort_by_key(|d| d.name.to_lowercase());
    for i in 0..APP_CHANNELS {
        if r.devices[i].is_some() {
            continue;
        }
        let word = CHANNELS[i].0;
        let named = pool.iter().find(|d| {
            let n = d.name.to_lowercase();
            n.contains("audiora") && n.contains(word) && !used.contains(&d.id)
        });
        // Devices named for another channel are kept for that channel.
        let generic = |d: &&&DeviceInfo| {
            let n = d.name.to_lowercase();
            !used.contains(&d.id) && !(n.contains("audiora") && CHANNELS.iter().any(|(k, _)| n.contains(k)))
        };
        let pick = named.or_else(|| pool.iter().find(generic));
        if let Some(d) = pick {
            r.devices[i] = Some(d.id.clone());
            r.auto[i] = true;
            used.insert(d.id.clone());
        }
    }
    r
}

/// Application → channel assignments. `prev` remembers the output the user had set in Windows
/// before Audiora took over, so letting go restores it.
#[derive(Default)]
pub struct Routes {
    map: BTreeMap<String, (String, Option<String>)>,
}

impl Routes {
    fn load(dir: &Path) -> Self {
        let v: Value = fs::read_to_string(dir.join("routes.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
        let mut map = BTreeMap::new();
        if let Some(o) = v["routes"].as_object() {
            for (app, r) in o {
                if let Some(ch) = r["channel"].as_str().filter(|c| channel_index(c).is_some_and(|i| i < APP_CHANNELS)) {
                    map.insert(app.clone(), (ch.to_owned(), opt(&r["prev"])));
                }
            }
        }
        Self { map }
    }

    fn save(&self, dir: &Path) -> Result<(), String> {
        let routes: Map<String, Value> =
            self.map.iter().map(|(a, (c, p))| (a.clone(), json!({ "channel": c, "prev": p }))).collect();
        write_json(&dir.join("routes.json"), &json!({ "version": 1, "routes": routes }))
    }
}

/// Per-game profiles: application → channel → preset name.
#[derive(Default)]
pub struct Profiles {
    map: BTreeMap<String, BTreeMap<String, String>>,
}

impl Profiles {
    fn load(dir: &Path) -> Self {
        let v: Value = fs::read_to_string(dir.join("app-profiles.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
        let mut map: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        if let Some(o) = v["profiles"].as_object() {
            for (app, per) in o {
                for (ch, name) in per.as_object().into_iter().flatten() {
                    if let (true, Some(n)) = (channel_index(ch).is_some(), name.as_str()) {
                        map.entry(app.clone()).or_default().insert(ch.clone(), n.to_owned());
                    }
                }
            }
        }
        Self { map }
    }

    fn save(&self, dir: &Path) -> Result<(), String> {
        write_json(&dir.join("app-profiles.json"), &json!({ "version": 1, "profiles": self.map }))
    }
}

pub struct Channels {
    dir: PathBuf,
    params: Arc<MixerParams>,
    pub config: Mutex<Config>,
    pub resolved: Resolved,
    /// Device id each application channel is tapping, if its strip is online.
    online: [Option<String>; APP_CHANNELS],
    errors: Vec<Option<String>>,
    routes: Mutex<Routes>,
    profiles: Mutex<Profiles>,
    pub presets: CPresets,
}

impl Channels {
    pub fn new(dir: &Path, params: Arc<MixerParams>, resolved: Resolved, online: [Option<String>; APP_CHANNELS], errors: Vec<Option<String>>) -> Self {
        Self {
            dir: dir.to_owned(),
            presets: CPresets::new(dir),
            config: Mutex::new(Config::load(dir)),
            routes: Mutex::new(Routes::load(dir)),
            profiles: Mutex::new(Profiles::load(dir)),
            params,
            resolved,
            online,
            errors,
        }
    }

    fn device_of(&self, key: &str) -> Option<&String> {
        channel_index(key).and_then(|i| self.online.get(i)).and_then(Option::as_ref)
    }

    /// Writes new device choices; they take effect when the engine restarts.
    pub fn config_set(&self, req: &Value) -> Result<Value, String> {
        let mut cfg = self.config.lock().map_err(|_| "config lock poisoned")?;
        let mut next = cfg.clone();
        let take = |v: &Value| -> Option<Option<String>> { if v.is_null() { Some(None) } else { v.as_str().map(|s| Some(s.to_owned())) } };
        for (field, slot) in [("master_out", &mut next.master_out), ("mic_in", &mut next.mic_in), ("mic_out", &mut next.mic_out)] {
            if let Some(v) = req.get(field) {
                *slot = take(v).ok_or_else(|| format!("\"{field}\" must be a device id or null"))?;
            }
        }
        if let Some(d) = req.get("devices").and_then(Value::as_object) {
            for (k, v) in d {
                let i = channel_index(k).filter(|i| *i < APP_CHANNELS).ok_or_else(|| format!("unknown channel \"{k}\""))?;
                next.devices[i] = take(v).ok_or("device must be an id or null")?;
            }
        }
        let used: Vec<&String> = next.devices.iter().flatten().collect();
        if used.iter().enumerate().any(|(i, a)| used[i + 1..].contains(a)) {
            return Err("two channels cannot share one device".into());
        }
        let clash = |id: &Option<String>| id.as_ref().is_some_and(|i| used.contains(&i));
        if clash(&next.master_out) {
            return Err("the speakers cannot also be a channel device".into());
        }
        next.save(&self.dir)?;
        *cfg = next;
        Ok(json!({ "restart": true }))
    }

    pub fn channels_json(&self, devs: &[DeviceInfo]) -> Value {
        let name_of = |id: &Option<String>| id.as_ref().and_then(|i| devs.iter().find(|d| &d.id == i).map(|d| d.name.clone()));
        let cfg = self.config.lock().map(|c| c.clone()).unwrap_or_default();
        let list: Vec<Value> = CHANNELS
            .iter()
            .enumerate()
            .map(|(i, (key, name))| {
                if i < APP_CHANNELS {
                    json!({
                        "key": key, "name": name, "strip": i, "kind": "apps",
                        "device_id": self.resolved.devices[i], "device_name": name_of(&self.resolved.devices[i]),
                        "auto": self.resolved.auto[i], "online": self.online[i].is_some(), "error": self.errors[i],
                    })
                } else {
                    json!({
                        "key": key, "name": name, "strip": i, "kind": "mic",
                        "device_id": self.resolved.mic_in, "device_name": name_of(&self.resolved.mic_in),
                        "auto": cfg.mic_in.is_none(), "online": self.errors[i].is_none() && self.resolved.mic_in.is_some(), "error": self.errors[i],
                    })
                }
            })
            .collect();
        json!({
            "channels": list,
            "master_out": { "device_id": self.resolved.master_out, "device_name": name_of(&self.resolved.master_out), "auto": cfg.master_out.is_none() },
            "mic_out": { "device_id": self.resolved.mic_out, "device_name": name_of(&self.resolved.mic_out) },
            "config": cfg.to_json(),
        })
    }

    pub fn routes_json(&self) -> Value {
        let r = self.routes.lock().map(|r| r.map.clone()).unwrap_or_default();
        let routes: Map<String, Value> = r.into_iter().map(|(a, (c, _))| (a, json!(c))).collect();
        json!({ "routes": routes })
    }

    pub fn route_of(&self, app: &str) -> Option<String> {
        self.routes.lock().ok()?.map.get(&app.to_lowercase()).map(|(c, _)| c.clone())
    }

    /// Which channel an application's audio currently plays into (by the device it uses).
    pub fn channel_of_device(&self, device_id: &str) -> Option<&'static str> {
        (0..APP_CHANNELS).find(|i| self.online[*i].as_deref() == Some(device_id)).map(|i| CHANNELS[i].0)
    }

    /// Assigns an application to a channel (`None` lets it go back to where it was).
    pub fn route_app(&self, app: &str, channel: Option<&str>) -> Result<Value, String> {
        let key = app.to_lowercase();
        if key.is_empty() || key == "system sounds" {
            return Err("that application cannot be routed".into());
        }
        let target = match channel {
            None => None,
            Some(c) => {
                let i = channel_index(c).filter(|i| *i < APP_CHANNELS).ok_or_else(|| format!("\"{c}\" is not an application channel"))?;
                Some((c.to_owned(), self.online[i].clone().ok_or_else(|| format!("the {} channel has no working device; pick one in Setup", CHANNELS[i].1))?))
            }
        };
        let sessions = audiora_hal::list_sessions().unwrap_or_default();
        let pids: Vec<u32> = sessions.iter().filter(|s| !s.system_sounds && s.name.to_lowercase() == key).map(|s| s.pid).collect();
        let mut routes = self.routes.lock().map_err(|_| "routes lock poisoned")?;
        match target {
            Some((ch, dev)) => {
                // Remember what the user had, but never remember our own device as "previous".
                let prev = routes.map.get(&key).map(|(_, p)| p.clone()).unwrap_or_else(|| {
                    pids.first().and_then(|p| audiora_hal::get_app_output(*p).ok().flatten()).filter(|d| !self.online.iter().flatten().any(|o| o == d))
                });
                let mut err = None;
                for pid in &pids {
                    if let Err(e) = audiora_hal::set_app_output(*pid, Some(&dev)) {
                        err = Some(e.to_string());
                    }
                }
                routes.map.insert(key.clone(), (ch, prev));
                routes.save(&self.dir)?;
                match err {
                    Some(e) => Err(e),
                    None => Ok(json!({ "app": app, "channel": channel, "live": !pids.is_empty() })),
                }
            }
            None => {
                let prev = routes.map.remove(&key).and_then(|(_, p)| p);
                for pid in &pids {
                    let _ = audiora_hal::set_app_output(*pid, prev.as_deref());
                }
                routes.save(&self.dir)?;
                Ok(json!({ "app": app, "channel": Value::Null, "live": !pids.is_empty() }))
            }
        }
    }

    /// Sends every running routed application back to where it was. Called on shutdown so that
    /// closing Audiora never leaves programs playing into a device nobody is listening to.
    pub fn release_all(&self) {
        let Ok(routes) = self.routes.lock() else { return };
        let sessions = audiora_hal::list_sessions().unwrap_or_default();
        for (app, (_, prev)) in &routes.map {
            for s in sessions.iter().filter(|s| !s.system_sounds && &s.name.to_lowercase() == app) {
                let _ = audiora_hal::set_app_output(s.pid, prev.as_deref());
            }
        }
    }

    pub fn profiles_json(&self) -> Value {
        let p = self.profiles.lock().map(|p| p.map.clone()).unwrap_or_default();
        json!({ "profiles": p })
    }

    pub fn profile_set(&self, app: &str, channel: &str, preset: Option<&str>) -> Result<Value, String> {
        channel_index(channel).ok_or_else(|| format!("unknown channel \"{channel}\""))?;
        let key = app.to_lowercase();
        if key.is_empty() {
            return Err("missing application name".into());
        }
        let mut p = self.profiles.lock().map_err(|_| "profiles lock poisoned")?;
        match preset {
            Some(name) => {
                self.presets.exists(channel, name).then_some(()).ok_or_else(|| format!("no preset named \"{name}\" on {channel}"))?;
                p.map.entry(key).or_default().insert(channel.to_owned(), name.to_owned());
            }
            None => {
                if let Some(per) = p.map.get_mut(&key) {
                    per.remove(channel);
                    if per.is_empty() {
                        p.map.remove(&key);
                    }
                }
            }
        }
        p.save(&self.dir)?;
        Ok(json!({}))
    }

    fn apply_profile(&self, app: &str) {
        let per = self.profiles.lock().ok().and_then(|p| p.map.get(&app.to_lowercase()).cloned());
        for (channel, preset) in per.into_iter().flatten() {
            if let Some(strip) = channel_index(&channel) {
                if let Err(e) = self.presets.load(&channel, &preset, strip, &self.params) {
                    eprintln!("profile for {app}: {e}");
                }
            }
        }
    }

    /// Keeps routed applications on their channel (they re-open their audio on every launch) and
    /// applies a game's profile the moment it starts making sound. Runs until `stop`.
    pub fn watch(self: &Arc<Self>, stop: Arc<AtomicBool>) -> std::thread::JoinHandle<()> {
        let me = self.clone();
        std::thread::spawn(move || {
            let mut seen: HashSet<u32> = HashSet::new();
            let own = std::process::id();
            while !stop.load(Ordering::Relaxed) {
                if let Ok(sessions) = audiora_hal::list_sessions() {
                    let mut now: HashMap<u32, String> = HashMap::new();
                    for s in sessions.iter().filter(|s| !s.system_sounds && s.pid != own && s.pid != 0) {
                        now.insert(s.pid, s.name.to_lowercase());
                    }
                    for (pid, name) in &now {
                        if let Some(dev) = me.route_of(name).and_then(|c| me.device_of(&c).cloned()) {
                            if audiora_hal::get_app_output(*pid).ok().flatten().as_ref() != Some(&dev) {
                                let _ = audiora_hal::set_app_output(*pid, Some(&dev));
                            }
                        }
                        if seen.insert(*pid) {
                            me.apply_profile(name);
                        }
                    }
                    seen.retain(|p| now.contains_key(p));
                }
                for _ in 0..15 {
                    if stop.load(Ordering::Relaxed) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(name: &str, flow: Flow, id: &str, default: bool) -> DeviceInfo {
        DeviceInfo { id: id.into(), name: name.into(), flow, is_default: default, sample_rate: 48000, channels: 2 }
    }

    #[test]
    fn master_skips_a_virtual_default() {
        let devs = [dev("Sonar - Gaming", Flow::Render, "v", true), dev("Headphones", Flow::Render, "h", false)];
        let r = resolve(&Config::default(), &devs, &[]);
        assert_eq!(r.master_out.as_deref(), Some("h"));
    }

    #[test]
    fn automatic_choices_prefer_headphones_and_avoid_hands_free_mics() {
        let mut headset = dev("Headset (JBL)", Flow::Capture, "hf", false);
        headset.sample_rate = 8000;
        let devs = [
            dev("Sonar - Gaming", Flow::Render, "v", true),
            dev("1 - PHL 243V7 (AMD High Definition Audio Device)", Flow::Render, "hdmi", false),
            dev("Realtek Digital Output", Flow::Render, "spdif", false),
            dev("Headphones (JBL Quantum 360)", Flow::Render, "jbl", false),
            dev("Sonar - Microphone", Flow::Capture, "vmic", true),
            headset,
            dev("Microphone (Realtek)", Flow::Capture, "mic", false),
        ];
        let r = resolve(&Config::default(), &devs, &[]);
        assert_eq!(r.master_out.as_deref(), Some("jbl"));
        assert_eq!(r.mic_in.as_deref(), Some("mic"));
    }

    #[test]
    fn channel_devices_prefer_audiora_names_then_free_cables() {
        let devs = [
            dev("Speakers", Flow::Render, "s", true),
            dev("Audiora - Chat", Flow::Render, "chat", false),
            dev("CABLE Input (VB-Audio Virtual Cable)", Flow::Render, "c1", false),
            dev("CABLE-A Input (VB-Audio Virtual Cable A)", Flow::Render, "c2", false),
        ];
        let r = resolve(&Config::default(), &devs, &[]);
        assert_eq!(r.devices[1].as_deref(), Some("chat"));
        assert!(r.devices[0].is_some() && r.devices[0] != r.devices[1]);
        assert!(r.auto[0] && r.auto[1]);
    }

    #[test]
    fn devices_owned_by_a_running_mixer_are_left_alone() {
        let devs = [
            dev("Speakers", Flow::Render, "s", true),
            dev("SteelSeries Sonar - Aux", Flow::Render, "sonar", false),
            dev("Voicemeeter In 1", Flow::Render, "vm", false),
        ];
        let running = vec!["steelseriessonar".to_string(), "voicemeeter8".to_string()];
        let r = resolve(&Config::default(), &devs, &running);
        assert!(r.devices.iter().all(Option::is_none));
        let r = resolve(&Config::default(), &devs, &[]);
        assert!(r.devices.iter().any(Option::is_some));
    }

    #[test]
    fn explicit_choice_beats_auto_and_must_exist() {
        let devs = [dev("Speakers", Flow::Render, "s", true), dev("CABLE Input", Flow::Render, "c1", false)];
        let mut cfg = Config::default();
        cfg.devices[2] = Some("c1".into());
        cfg.devices[0] = Some("gone".into());
        let r = resolve(&cfg, &devs, &[]);
        assert_eq!(r.devices[2].as_deref(), Some("c1"));
        assert!(!r.auto[2]);
        assert_ne!(r.devices[0].as_deref(), Some("gone"));
    }

    #[test]
    fn config_roundtrips_through_json() {
        let dir = std::env::temp_dir().join(format!("audiora-chan-{}", std::process::id()));
        let mut c = Config { master_out: Some("m".into()), ..Default::default() };
        c.devices[3] = Some("aux".into());
        c.save(&dir).unwrap();
        assert_eq!(Config::load(&dir), c);
        fs::remove_dir_all(dir).ok();
    }
}
