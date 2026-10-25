//! Per-channel presets: the sound-shaping settings of one channel (EQ, volume boost, smart
//! volume, noise suppression, gate, compressor) — never its fader, mute or routing. Factory
//! presets ship with the engine; user presets live in `channel-presets/<channel>.json`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use audiora_core::{MixerParams, EQ_BANDS};
use serde_json::{json, Map, Value};

type Settings = Vec<(String, f64)>;

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().count() <= 40
        && !name.starts_with(' ')
        && name.chars().all(|c| c.is_alphanumeric() || matches!(c, ' ' | '_' | '-' | '&' | '\'' | '.'))
}

/// Strip-relative paths a preset may contain.
fn is_sound_path(rel: &str) -> bool {
    matches!(rel, "boost_db" | "denoise" | "denoise_mix")
        || ["eq/", "smart/", "gate/", "comp/", "lowcut/", "deess/", "limiter/"].iter().any(|p| rel.starts_with(p))
}

/// The state every preset starts from, so applying one never inherits the last one's leftovers.
fn baseline() -> Settings {
    let mut v: Settings = (0..EQ_BANDS).map(|b| (format!("eq/{b}/gain_db"), 0.0)).collect();
    for (p, x) in [
        ("boost_db", 0.0),
        ("denoise", 0.0),
        ("denoise_mix", 1.0),
        ("lowcut/enabled", 0.0),
        ("lowcut/freq", 80.0),
        ("smart/enabled", 0.0),
        ("smart/target_db", -24.0),
        ("gate/enabled", 0.0),
        ("gate/hold_ms", 80.0),
        ("deess/enabled", 0.0),
        ("deess/threshold_db", -30.0),
        ("comp/enabled", 0.0),
        ("limiter/enabled", 0.0),
        ("limiter/ceiling_db", -1.0),
    ] {
        v.push((p.into(), x));
    }
    v
}

fn eq(g: [f64; 6]) -> Settings {
    g.iter().enumerate().map(|(b, v)| (format!("eq/{b}/gain_db"), *v)).collect()
}

fn with(mut base: Settings, extra: &[(&str, f64)]) -> Settings {
    base.extend(extra.iter().map(|(p, v)| ((*p).to_owned(), *v)));
    base
}

const SMART: &str = "smart/enabled";

/// A broadcast-style voice chain, in signal order: low cut -> noise suppression -> gate -> EQ ->
/// de-esser -> compressor -> limiter. The gate is used as a gentle expander (limited range, long
/// hold) so pauses get quieter without words being chopped.
struct Voice {
    lowcut_hz: f64,
    denoise: f64,
    gate_db: Option<f64>,
    gate_range: f64,
    eq: [f64; 6],
    deess_db: Option<f64>,
    comp: (f64, f64, f64),
}

fn voice(v: Voice) -> Settings {
    let mut out = eq(v.eq);
    let mut add = |p: &str, x: f64| out.push((p.to_owned(), x));
    add("lowcut/enabled", 1.0);
    add("lowcut/freq", v.lowcut_hz);
    add("denoise", (v.denoise > 0.0) as u8 as f64);
    add("denoise_mix", v.denoise.max(0.0));
    if let Some(t) = v.gate_db {
        add("gate/enabled", 1.0);
        add("gate/threshold_db", t);
        add("gate/range_db", v.gate_range);
        add("gate/attack_ms", 2.0);
        add("gate/release_ms", 180.0);
        add("gate/hold_ms", 140.0);
    }
    if let Some(t) = v.deess_db {
        add("deess/enabled", 1.0);
        add("deess/threshold_db", t);
    }
    let (threshold, ratio, makeup) = v.comp;
    for (p, x) in [
        ("comp/enabled", 1.0),
        ("comp/threshold_db", threshold),
        ("comp/ratio", ratio),
        ("comp/attack_ms", 6.0),
        ("comp/release_ms", 120.0),
        ("comp/makeup_db", makeup),
        ("comp/knee_db", 8.0),
        ("limiter/enabled", 1.0),
        ("limiter/ceiling_db", -1.0),
    ] {
        add(p, x);
    }
    out
}

/// Built-in presets per channel. EQ order: sub bass, bass, low mids, mids, upper mids, highs.
fn factory(channel: &str) -> Vec<(&'static str, Settings)> {
    match channel {
        "game" => vec![
            ("Flat", vec![]),
            ("Footsteps", eq([-6.0, -3.0, -2.0, 0.0, 4.0, 3.0])),
            ("Immersive", eq([5.0, 3.0, 0.0, -1.0, 1.0, 3.0])),
            ("Competitive", with(eq([-8.0, -4.0, -2.0, 1.0, 5.0, 4.0]), &[(SMART, 1.0), ("smart/target_db", -22.0)])),
            ("Bass boost", eq([8.0, 5.0, 1.0, 0.0, 0.0, 0.0])),
            ("Night mode", with(eq([-4.0, -2.0, 0.0, 0.0, 0.0, -1.0]), &[(SMART, 1.0), ("smart/target_db", -28.0)])),
        ],
        "chat" => vec![
            ("Flat", vec![]),
            ("Voice clarity", eq([-8.0, -3.0, 0.0, 3.0, 4.0, 1.0])),
            ("Warm voice", eq([2.0, 3.0, 1.0, 0.0, -1.0, -2.0])),
            ("Loud & clear", with(eq([-6.0, -2.0, 0.0, 3.0, 3.0, 0.0]), &[(SMART, 1.0), ("smart/target_db", -20.0), ("boost_db", 3.0)])),
        ],
        "media" => vec![
            ("Flat", vec![]),
            ("Music", eq([3.0, 2.0, 0.0, -1.0, 1.0, 3.0])),
            ("Movies", eq([4.0, 3.0, 0.0, 1.0, 2.0, 1.0])),
            ("Speech", eq([-8.0, -2.0, 0.0, 3.0, 3.0, -1.0])),
            ("Vocal boost", eq([-3.0, -1.0, 0.0, 4.0, 3.0, 0.0])),
            ("Late night", with(eq([-3.0, -1.0, 0.0, 0.0, 0.0, -1.0]), &[(SMART, 1.0), ("smart/target_db", -28.0)])),
        ],
        "aux" => vec![
            ("Flat", vec![]),
            ("Warm", eq([3.0, 2.0, 1.0, 0.0, -1.0, -3.0])),
            ("Bright", eq([-2.0, 0.0, 0.0, 1.0, 3.0, 4.0])),
            ("Quiet", with(vec![], &[(SMART, 1.0), ("smart/target_db", -30.0)])),
        ],
        "mic" => vec![
            ("Flat", vec![]),
            (
                "Studio voice",
                voice(Voice { lowcut_hz: 80.0, denoise: 0.85, gate_db: Some(-50.0), gate_range: 18.0, eq: [-2.0, 1.5, -2.0, 1.0, 2.5, 2.0], deess_db: Some(-32.0), comp: (-20.0, 3.0, 4.0) }),
            ),
            (
                "Broadcast",
                voice(Voice { lowcut_hz: 100.0, denoise: 0.9, gate_db: Some(-46.0), gate_range: 24.0, eq: [-3.0, 3.0, -3.0, 0.0, 3.0, 2.5], deess_db: Some(-30.0), comp: (-24.0, 4.0, 6.0) }),
            ),
            (
                "Podcast",
                voice(Voice { lowcut_hz: 80.0, denoise: 0.8, gate_db: Some(-52.0), gate_range: 15.0, eq: [0.0, 2.0, -2.0, 1.0, 2.0, 1.0], deess_db: Some(-32.0), comp: (-20.0, 3.0, 4.0) }),
            ),
            (
                "Gaming clear",
                voice(Voice { lowcut_hz: 120.0, denoise: 1.0, gate_db: Some(-44.0), gate_range: 30.0, eq: [-6.0, -2.0, -1.0, 2.0, 3.0, 0.0], deess_db: Some(-30.0), comp: (-22.0, 4.0, 5.0) }),
            ),
            (
                "Noisy room",
                voice(Voice { lowcut_hz: 120.0, denoise: 1.0, gate_db: Some(-38.0), gate_range: 40.0, eq: [-8.0, -3.0, 0.0, 2.0, 3.0, 0.0], deess_db: Some(-28.0), comp: (-22.0, 4.0, 4.0) }),
            ),
            (
                "Natural",
                voice(Voice { lowcut_hz: 70.0, denoise: 0.6, gate_db: None, gate_range: 0.0, eq: [0.0; 6], deess_db: None, comp: (-16.0, 2.0, 2.0) }),
            ),
        ],
        _ => vec![],
    }
}

pub struct CPresets {
    dir: PathBuf,
}

#[derive(Default)]
struct File {
    favorites: Vec<String>,
    presets: BTreeMap<String, Settings>,
}

impl CPresets {
    pub fn new(data_dir: &Path) -> Self {
        Self { dir: data_dir.join("channel-presets") }
    }

    fn path(&self, channel: &str) -> Result<PathBuf, String> {
        if crate::channels::channel_index(channel).is_none() {
            return Err(format!("unknown channel \"{channel}\""));
        }
        Ok(self.dir.join(format!("{channel}.json")))
    }

    fn read(&self, channel: &str) -> Result<File, String> {
        let path = self.path(channel)?;
        let v: Value = fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
        let mut f = File {
            favorites: v["favorites"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_owned)).collect(),
            ..Default::default()
        };
        for (name, p) in v["presets"].as_object().into_iter().flatten() {
            let s: Settings = p.as_object().into_iter().flatten().filter_map(|(k, x)| Some((k.clone(), x.as_f64()?))).collect();
            f.presets.insert(name.clone(), s);
        }
        Ok(f)
    }

    fn write(&self, channel: &str, f: &File) -> Result<(), String> {
        let path = self.path(channel)?;
        fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let presets: Map<String, Value> = f
            .presets
            .iter()
            .map(|(n, s)| (n.clone(), Value::Object(s.iter().map(|(k, v)| (k.clone(), json!(v))).collect())))
            .collect();
        let body = json!({ "version": 1, "favorites": f.favorites, "presets": presets });
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(&body).unwrap()).map_err(|e| e.to_string())?;
        fs::rename(&tmp, path).map_err(|e| e.to_string())
    }

    pub fn exists(&self, channel: &str, name: &str) -> bool {
        factory(channel).iter().any(|(n, _)| *n == name) || self.read(channel).is_ok_and(|f| f.presets.contains_key(name))
    }

    pub fn list(&self, channel: &str) -> Result<Value, String> {
        let f = self.read(channel)?;
        Ok(json!({
            "channel": channel,
            "factory": factory(channel).iter().map(|(n, _)| *n).collect::<Vec<_>>(),
            "user": f.presets.keys().collect::<Vec<_>>(),
            "favorites": f.favorites,
        }))
    }

    pub fn save(&self, channel: &str, name: &str, strip: usize, params: &MixerParams) -> Result<Value, String> {
        if !valid_name(name) {
            return Err("preset names may use letters, digits, space and - _ & ' . (max 40)".into());
        }
        if factory(channel).iter().any(|(n, _)| n.eq_ignore_ascii_case(name)) {
            return Err("that name belongs to a built-in preset".into());
        }
        let prefix = format!("strip/{strip}/");
        let settings: Settings = params
            .settings()
            .into_iter()
            .filter_map(|(p, v)| p.strip_prefix(&prefix).filter(|rel| is_sound_path(rel)).map(|rel| (rel.to_owned(), v)))
            .collect();
        let mut f = self.read(channel)?;
        let n = settings.len();
        f.presets.insert(name.to_owned(), settings);
        self.write(channel, &f)?;
        Ok(json!({ "name": name, "saved": n }))
    }

    pub fn load(&self, channel: &str, name: &str, strip: usize, params: &MixerParams) -> Result<Value, String> {
        let settings = match factory(channel).into_iter().find(|(n, _)| *n == name) {
            Some((_, s)) => s,
            None => self.read(channel)?.presets.remove(name).ok_or_else(|| format!("no preset named \"{name}\""))?,
        };
        let mut applied = 0;
        for (rel, v) in baseline().into_iter().chain(settings) {
            if is_sound_path(&rel) && params.set(&format!("strip/{strip}/{rel}"), v).is_ok() {
                applied += 1;
            }
        }
        Ok(json!({ "name": name, "applied": applied }))
    }

    pub fn delete(&self, channel: &str, name: &str) -> Result<Value, String> {
        let mut f = self.read(channel)?;
        f.presets.remove(name).ok_or_else(|| format!("no user preset named \"{name}\""))?;
        f.favorites.retain(|n| n != name);
        self.write(channel, &f)?;
        Ok(json!({ "name": name }))
    }

    pub fn favorite(&self, channel: &str, name: &str, on: bool) -> Result<Value, String> {
        if !self.exists(channel, name) {
            return Err(format!("no preset named \"{name}\""));
        }
        let mut f = self.read(channel)?;
        f.favorites.retain(|n| n != name);
        if on {
            f.favorites.push(name.to_owned());
        }
        self.write(channel, &f)?;
        Ok(json!({ "name": name, "favorite": on }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (CPresets, PathBuf) {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("audiora-cp-{}-{n}", std::process::id()));
        (CPresets::new(&dir), dir)
    }

    #[test]
    fn factory_presets_apply_and_reset_leftovers() {
        let (s, dir) = store();
        let p = MixerParams::new(2, 1);
        s.load("mic", "Studio voice", 1, &p).unwrap();
        assert_eq!(p.get("strip/1/denoise").unwrap(), 1.0);
        assert_eq!(p.get("strip/1/gate/enabled").unwrap(), 1.0);
        assert_eq!(p.get("strip/1/comp/enabled").unwrap(), 1.0);
        assert_eq!(p.get("strip/0/denoise").unwrap(), 0.0, "other strips untouched");
        s.load("mic", "Flat", 1, &p).unwrap();
        assert_eq!(p.get("strip/1/denoise").unwrap(), 0.0);
        assert_eq!(p.get("strip/1/comp/enabled").unwrap(), 0.0);
        assert_eq!(p.get("strip/1/eq/0/gain_db").unwrap(), 0.0);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn every_factory_preset_loads_cleanly() {
        let (s, dir) = store();
        let p = MixerParams::new(5, 1);
        for (i, (key, _)) in crate::channels::CHANNELS.iter().enumerate() {
            for (name, settings) in factory(key) {
                let r = s.load(key, name, i, &p).unwrap();
                assert!(r["applied"].as_u64().unwrap() as usize >= baseline().len() + settings.len() - 1, "{key}/{name}");
            }
        }
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn user_presets_roundtrip_exclude_the_fader_and_support_favorites() {
        let (s, dir) = store();
        let p = MixerParams::new(1, 1);
        p.set("strip/0/gain_db", -9.0).unwrap();
        p.set("strip/0/eq/2/gain_db", 4.5).unwrap();
        p.set("strip/0/boost_db", 3.0).unwrap();
        s.save("game", "My tune", 0, &p).unwrap();
        assert!(s.save("game", "Footsteps", 0, &p).is_err(), "built-in names are reserved");
        assert!(s.save("game", "../x", 0, &p).is_err());

        let q = MixerParams::new(1, 1);
        q.set("strip/0/gain_db", -3.0).unwrap();
        s.load("game", "My tune", 0, &q).unwrap();
        assert_eq!(q.get("strip/0/eq/2/gain_db").unwrap(), 4.5);
        assert_eq!(q.get("strip/0/boost_db").unwrap(), 3.0);
        assert_eq!(q.get("strip/0/gain_db").unwrap(), -3.0, "volume is not part of a preset");

        s.favorite("game", "My tune", true).unwrap();
        s.favorite("game", "Footsteps", true).unwrap();
        let l = s.list("game").unwrap();
        assert_eq!(l["favorites"], json!(["My tune", "Footsteps"]));
        assert_eq!(l["user"], json!(["My tune"]));
        s.delete("game", "My tune").unwrap();
        assert_eq!(s.list("game").unwrap()["favorites"], json!(["Footsteps"]));
        assert!(s.delete("game", "Footsteps").is_err());
        assert!(s.list("nope").is_err());
        fs::remove_dir_all(dir).ok();
    }
}
