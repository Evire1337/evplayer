//! The parametric equalizer (src/equalizer/eqModel.ts): bands, presets, and the response
//! curve. The filters are the Audio EQ Cookbook's (the shelf slope fixed at S = 1, as Web
//! Audio's BiquadFilterNode), so the curve drawn on screen is the curve heard, and a preset
//! saved by the Tauri build sounds the same here.

use serde::{Deserialize, Serialize};

pub const MIN_FREQ: f64 = 20.0;
pub const MAX_FREQ: f64 = 20000.0;
/// how far a band can boost or cut; big enough for a real bass boost
pub const MAX_GAIN: f64 = 24.0;
pub const MIN_PREAMP: f64 = -24.0;
pub const MAX_PREAMP: f64 = 6.0;
pub const MIN_Q: f64 = 0.3;
pub const MAX_Q: f64 = 8.0;
/// the most bands any mode has
pub const MAX_BANDS: usize = 10;
pub const ADVANCED_FREQS: [f64; 10] = [31.0, 62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BandType {
    Lowshelf,
    #[default]
    Peaking,
    Highshelf,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Band {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: BandType,
    pub frequency: f64,
    pub gain: f64,
    pub q: f64,
    pub enabled: bool,
}

fn band(id: &str, kind: BandType, frequency: f64, q: f64) -> Band {
    Band { id: id.into(), kind, frequency, gain: 0.0, q, enabled: true }
}

pub fn default_bands() -> Vec<Band> {
    vec![
        band("b1", BandType::Lowshelf, 60.0, 0.71),
        band("b2", BandType::Peaking, 150.0, 1.0),
        band("b3", BandType::Peaking, 500.0, 1.0),
        band("b4", BandType::Peaking, 1500.0, 1.0),
        band("b5", BandType::Peaking, 4000.0, 1.0),
        band("b6", BandType::Highshelf, 10000.0, 0.71),
    ]
}

pub fn advanced_bands() -> Vec<Band> {
    ADVANCED_FREQS
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let edge = i == 0 || i == ADVANCED_FREQS.len() - 1;
            let kind = if i == 0 { BandType::Lowshelf } else if i == ADVANCED_FREQS.len() - 1 { BandType::Highshelf } else { BandType::Peaking };
            band(&format!("a{}", i + 1), kind, *f, if edge { 0.71 } else { 1.4 })
        })
        .collect()
}

/// (id, name, gains of the six bands)
pub const PRESETS: [(&str, &str, [f64; 6]); 6] = [
    ("flat", "Ровно", [0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
    ("rich", "Насыщенно", [4.0, 1.5, -1.5, 0.0, 2.0, 3.5]),
    ("bass", "Бас", [6.0, 2.5, 0.0, 0.0, 0.0, 0.0]),
    ("vocal", "Вокал", [-1.0, -2.0, 0.0, 3.0, 2.5, 0.5]),
    ("warm", "Тепло", [3.0, 1.0, 1.5, 0.0, -1.0, -2.0]),
    ("bright", "Ярко", [0.0, 0.0, -0.5, 1.0, 2.5, 5.0]),
];

/// a preset's bands in a mode: the ten-band version takes the six-band curve's level at
/// each of its frequencies, so both sound about the same
pub fn preset_bands(id: &str, advanced: bool) -> Vec<Band> {
    let gains = PRESETS.iter().find(|p| p.0 == id).map(|p| p.2).unwrap_or([0.0; 6]);
    let simple: Vec<Band> = default_bands().into_iter().zip(gains).map(|(mut b, g)| {
        b.gain = g;
        b
    }).collect();
    if !advanced {
        return simple;
    }
    advanced_bands().into_iter().map(|mut b| {
        b.gain = (total_response_db(&simple, b.frequency, 48000.0) * 2.0).round() / 2.0;
        b
    }).collect()
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Coefficients {
    pub b0: f64,
    pub b1: f64,
    pub b2: f64,
    pub a1: f64,
    pub a2: f64,
}

/// the biquad of a band, normalised by a0
pub fn coefficients(kind: BandType, frequency: f64, gain: f64, q: f64, rate: f64) -> Coefficients {
    let a = 10f64.powf(gain / 40.0);
    let w0 = 2.0 * std::f64::consts::PI * frequency.clamp(1.0, rate / 2.0 - 1.0) / rate;
    let (sin, cos) = w0.sin_cos();
    let (b0, b1, b2, a0, a1, a2) = match kind {
        BandType::Peaking => {
            let alpha = sin / (2.0 * q.max(1e-4));
            (1.0 + alpha * a, -2.0 * cos, 1.0 - alpha * a, 1.0 + alpha / a, -2.0 * cos, 1.0 - alpha / a)
        }
        BandType::Lowshelf | BandType::Highshelf => {
            let alpha = sin / 2.0 * std::f64::consts::SQRT_2;
            let sq = 2.0 * a.sqrt() * alpha;
            if kind == BandType::Lowshelf {
                (a * (a + 1.0 - (a - 1.0) * cos + sq), 2.0 * a * (a - 1.0 - (a + 1.0) * cos), a * (a + 1.0 - (a - 1.0) * cos - sq), a + 1.0 + (a - 1.0) * cos + sq, -2.0 * (a - 1.0 + (a + 1.0) * cos), a + 1.0 + (a - 1.0) * cos - sq)
            } else {
                (a * (a + 1.0 + (a - 1.0) * cos + sq), -2.0 * a * (a - 1.0 + (a + 1.0) * cos), a * (a + 1.0 + (a - 1.0) * cos - sq), a + 1.0 - (a - 1.0) * cos + sq, 2.0 * (a - 1.0 - (a + 1.0) * cos), a + 1.0 - (a - 1.0) * cos - sq)
            }
        }
    };
    Coefficients { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: a1 / a0, a2: a2 / a0 }
}

/// gain in dB this band applies at `frequency`
pub fn band_response_db(b: &Band, frequency: f64, rate: f64) -> f64 {
    if !b.enabled || b.gain == 0.0 {
        return 0.0;
    }
    let c = coefficients(b.kind, b.frequency, b.gain, b.q, rate);
    let w = 2.0 * std::f64::consts::PI * frequency / rate;
    let (s1, c1) = w.sin_cos();
    let (s2, c2) = (2.0 * w).sin_cos();
    let nr = c.b0 + c.b1 * c1 + c.b2 * c2;
    let ni = -(c.b1 * s1 + c.b2 * s2);
    let dr = 1.0 + c.a1 * c1 + c.a2 * c2;
    let di = -(c.a1 * s1 + c.a2 * s2);
    10.0 * ((nr * nr + ni * ni) / (dr * dr + di * di)).log10()
}

pub fn total_response_db(bands: &[Band], frequency: f64, rate: f64) -> f64 {
    bands.iter().map(|b| band_response_db(b, frequency, rate)).sum()
}

/// how much the curve boosts at its loudest point (for clip protection)
pub fn peak_boost_db(bands: &[Band]) -> f64 {
    (0..=120).map(|i| total_response_db(bands, MIN_FREQ * (MAX_FREQ / MIN_FREQ).powf(i as f64 / 120.0), 48000.0)).fold(0.0, f64::max)
}

pub fn freq_to_x(f: f64, width: f64) -> f64 {
    (f / MIN_FREQ).ln() / (MAX_FREQ / MIN_FREQ).ln() * width
}

pub fn x_to_freq(x: f64, width: f64) -> f64 {
    MIN_FREQ * (MAX_FREQ / MIN_FREQ).powf((x / width).clamp(0.0, 1.0))
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CustomPreset {
    pub id: String,
    pub name: String,
    pub bands: Vec<Band>,
    pub preamp: f64,
}

/// evplayer.equalizer.v1, as src/equalizer/eqStore.ts keeps it
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EqState {
    pub enabled: bool,
    /// keep every song at about the same loudness
    pub normalize: bool,
    /// clip protection: the level comes down by most of the boost, a limiter catches the rest
    pub protect: bool,
    pub preamp: f64,
    /// "simple" (six movable bands) or "advanced" (ten on the octaves)
    pub mode: String,
    pub other_bands: Vec<Band>,
    pub other_preset: String,
    pub bands: Vec<Band>,
    pub preset: String,
    pub custom_presets: Vec<CustomPreset>,
    #[serde(flatten)]
    pub rest: serde_json::Map<String, serde_json::Value>,
}

impl Default for EqState {
    fn default() -> Self {
        Self {
            enabled: false,
            normalize: true,
            protect: false,
            preamp: 0.0,
            mode: "simple".into(),
            other_bands: advanced_bands(),
            other_preset: "flat".into(),
            bands: default_bands(),
            preset: "flat".into(),
            custom_presets: vec![],
            rest: Default::default(),
        }
    }
}

impl EqState {
    pub fn advanced(&self) -> bool {
        self.mode == "advanced"
    }

    pub fn set_mode(&mut self, advanced: bool) {
        if advanced == self.advanced() {
            return;
        }
        let other = std::mem::take(&mut self.other_bands);
        let bands = if !other.is_empty() && (other.len() > 6) == advanced { other } else if advanced { advanced_bands() } else { default_bands() };
        self.other_bands = std::mem::replace(&mut self.bands, bands);
        let preset = if self.other_preset.is_empty() { "flat".to_string() } else { self.other_preset.clone() };
        self.other_preset = std::mem::replace(&mut self.preset, preset);
        self.mode = if advanced { "advanced".into() } else { "simple".into() };
    }

    pub fn apply_preset(&mut self, id: &str) {
        self.bands = preset_bands(id, self.advanced());
        self.preset = id.into();
        self.enabled = true;
    }

    pub fn apply_custom(&mut self, id: &str) {
        let Some(p) = self.custom_presets.iter().find(|p| p.id == id).cloned() else { return };
        let adv = p.bands.len() > 6;
        if adv != self.advanced() {
            self.other_bands = std::mem::take(&mut self.bands);
            self.other_preset = self.preset.clone();
            self.mode = if adv { "advanced".into() } else { "simple".into() };
        }
        self.bands = p.bands;
        self.preamp = p.preamp;
        self.preset = id.into();
        self.enabled = true;
    }

    pub fn save_preset(&mut self, name: &str) {
        let id = format!("cp_{}", crate::store::state::new_id("x").trim_start_matches("x_"));
        let name: String = name.trim().chars().take(32).collect();
        self.custom_presets.push(CustomPreset { id: id.clone(), name: if name.is_empty() { "Preset".into() } else { name }, bands: self.bands.clone(), preamp: self.preamp });
        self.preset = id;
    }
}

impl EqState {
    /// "Сохранить текущие настройки сюда"
    pub fn overwrite_preset(&mut self, id: &str) {
        let (bands, preamp) = (self.bands.clone(), self.preamp);
        if let Some(p) = self.custom_presets.iter_mut().find(|p| p.id == id) {
            p.bands = bands;
            p.preamp = preamp;
            self.preset = id.into();
        }
    }

    pub fn rename_preset(&mut self, id: &str, name: &str) {
        let name: String = name.trim().chars().take(32).collect();
        if let Some(p) = self.custom_presets.iter_mut().find(|p| p.id == id) {
            if !name.is_empty() {
                p.name = name;
            }
        }
    }

    pub fn delete_preset(&mut self, id: &str) {
        self.custom_presets.retain(|p| p.id != id);
        if self.preset == id {
            self.preset = "custom".into();
        }
    }

    /// a preset one place left or right among this mode's saved presets; the other mode's
    /// keep their places
    pub fn move_preset(&mut self, from: usize, to: usize) {
        let adv = self.advanced();
        let slots: Vec<usize> = self.custom_presets.iter().enumerate().filter(|(_, p)| (p.bands.len() > 6) == adv).map(|(i, _)| i).collect();
        let mut own: Vec<CustomPreset> = slots.iter().map(|&i| self.custom_presets[i].clone()).collect();
        if from >= own.len() {
            return;
        }
        let item = own.remove(from);
        own.insert(to.min(own.len()), item);
        for (k, slot) in slots.into_iter().enumerate() {
            self.custom_presets[slot] = own[k].clone();
        }
    }

    /// a preset pasted from a code: kept, and put on (switching the mode with it)
    pub fn import_preset(&mut self, name: String, bands: Vec<Band>, preamp: f64) -> String {
        let id = format!("cp_{}", crate::store::state::new_id("x").trim_start_matches("x_"));
        let adv = bands.len() > 6;
        if adv != self.advanced() {
            self.other_bands = std::mem::take(&mut self.bands);
            self.other_preset = self.preset.clone();
            self.mode = if adv { "advanced".into() } else { "simple".into() };
        }
        self.custom_presets.push(CustomPreset { id: id.clone(), name, bands: bands.clone(), preamp });
        self.bands = bands;
        self.preamp = preamp;
        self.preset = id.clone();
        self.enabled = true;
        id
    }

    /// a band changed by hand: the values kept in their ranges, the preset becomes "custom"
    pub fn update_band(&mut self, id: &str, f: impl FnOnce(&mut Band)) {
        if let Some(b) = self.bands.iter_mut().find(|b| b.id == id) {
            f(b);
            b.frequency = b.frequency.clamp(MIN_FREQ, MAX_FREQ);
            b.gain = (b.gain.clamp(-MAX_GAIN, MAX_GAIN) * 10.0).round() / 10.0;
            b.q = b.q.clamp(MIN_Q, MAX_Q);
        }
        self.preset = "custom".into();
    }

    /// "Сбросить": this mode's bands flat, no preamp
    pub fn reset(&mut self) {
        self.bands = if self.advanced() { advanced_bands() } else { default_bands() };
        self.preset = "flat".into();
        self.preamp = 0.0;
    }
}

const TYPES: [BandType; 3] = [BandType::Lowshelf, BandType::Peaking, BandType::Highshelf];

/// EVQ1.<base64url json>: an equalizer preset that can be pasted to a friend (presetCode.ts)
pub fn encode_preset_code(name: &str, bands: &[Band], preamp: f64) -> String {
    use base64::Engine;
    let b: Vec<serde_json::Value> = bands
        .iter()
        .map(|b| serde_json::json!([TYPES.iter().position(|t| *t == b.kind).unwrap_or(1), b.frequency.round() as i64, b.gain, (b.q * 100.0).round() / 100.0, if b.enabled { 1 } else { 0 }]))
        .collect();
    let wire = serde_json::json!({ "v": 1, "n": name, "p": preamp, "b": b });
    format!("EVQ1.{}", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(wire.to_string()))
}

/// the preset in a code: its name, bands and preamp
pub fn decode_preset_code(code: &str) -> Option<(String, Vec<Band>, f64)> {
    use base64::Engine;
    let body: String = code.trim().strip_prefix("EVQ1.")?.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(body.trim_end_matches('=')).ok()?;
    let wire: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    if wire["v"].as_i64() != Some(1) {
        return None;
    }
    let rows = wire["b"].as_array()?;
    let base = if rows.len() > 6 { advanced_bands() } else { default_bands() };
    let num = |v: &serde_json::Value| v.as_f64().filter(|x| *x != 0.0);
    let bands = base
        .into_iter()
        .enumerate()
        .map(|(i, mut band)| {
            if let Some(row) = rows.get(i).and_then(|r| r.as_array()) {
                if let Some(t) = row.first().and_then(|v| v.as_u64()).and_then(|t| TYPES.get(t as usize)) {
                    band.kind = *t;
                }
                band.frequency = row.get(1).and_then(num).unwrap_or(band.frequency).clamp(MIN_FREQ, MAX_FREQ);
                band.gain = row.get(2).and_then(num).unwrap_or(0.0).clamp(-MAX_GAIN, MAX_GAIN);
                band.q = row.get(3).and_then(num).unwrap_or(band.q).clamp(MIN_Q, MAX_Q);
                band.enabled = row.get(4).and_then(|v| v.as_f64()) != Some(0.0);
            }
            band
        })
        .collect();
    let name: String = wire["n"].as_str().filter(|n| !n.is_empty()).unwrap_or("Preset").chars().take(32).collect();
    let preamp = wire["p"].as_f64().unwrap_or(0.0).clamp(MIN_PREAMP, MAX_PREAMP);
    Some((name, bands, preamp))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_preset_code_comes_back_as_it_went() {
        let mut bands = default_bands();
        bands[1].gain = 3.5;
        bands[2].kind = BandType::Highshelf;
        bands[3].enabled = false;
        let code = encode_preset_code("Мой", &bands, -2.0);
        assert!(code.starts_with("EVQ1."));
        let (name, back, preamp) = decode_preset_code(&code).unwrap();
        assert_eq!(name, "Мой");
        assert_eq!(preamp, -2.0);
        assert_eq!(back[1].gain, 3.5);
        assert_eq!(back[2].kind, BandType::Highshelf);
        assert!(!back[3].enabled);
        assert!(decode_preset_code("hello").is_none());
    }

    use super::*;

    #[test]
    fn a_peak_reaches_its_gain_at_its_frequency() {
        let mut b = band("x", BandType::Peaking, 1000.0, 1.0);
        b.gain = 6.0;
        assert!((band_response_db(&b, 1000.0, 48000.0) - 6.0).abs() < 0.01);
        assert!(band_response_db(&b, 100.0, 48000.0).abs() < 0.5);
    }

    #[test]
    fn shelves_boost_their_side() {
        let mut low = band("l", BandType::Lowshelf, 100.0, 0.71);
        low.gain = 6.0;
        assert!(band_response_db(&low, 30.0, 48000.0) > 5.0);
        assert!(band_response_db(&low, 5000.0, 48000.0).abs() < 0.3);
        let mut high = band("h", BandType::Highshelf, 5000.0, 0.71);
        high.gain = -6.0;
        assert!(band_response_db(&high, 15000.0, 48000.0) < -5.0);
    }

    #[test]
    fn flat_is_flat_and_a_ten_band_preset_has_the_same_shape() {
        assert_eq!(total_response_db(&default_bands(), 1234.0, 48000.0), 0.0);
        let six = preset_bands("bass", false);
        let ten = preset_bands("bass", true);
        assert_eq!(ten.len(), 10);
        // the same shape: a strong low end, nothing at the top (as eqModel.test.ts checks)
        assert!(ten[0].gain > 4.0);
        assert!(ten[9].gain.abs() < 1.0);
        assert!(total_response_db(&six, 1000.0, 48000.0).abs() < 1.0);
        assert!((x_to_freq(freq_to_x(440.0, 300.0), 300.0) - 440.0).abs() < 1e-6);
    }

    #[test]
    fn switching_modes_keeps_each_mode_s_bands_and_preset() {
        let mut s = EqState::default();
        s.apply_preset("vocal");
        s.set_mode(true);
        assert_eq!(s.bands.len(), 10);
        assert_eq!(s.preset, "flat");
        s.set_mode(false);
        assert_eq!(s.preset, "vocal");
        assert_eq!(s.bands.len(), 6);
    }

    #[test]
    fn reads_the_page_s_saved_equalizer() {
        let json = r#"{"enabled":true,"normalize":false,"protect":true,"preamp":-2,"mode":"simple","bands":[{"id":"b1","type":"lowshelf","frequency":60,"gain":4,"q":0.71,"enabled":true}],"preset":"custom","customPresets":[],"otherBands":[],"otherPreset":"flat"}"#;
        let s: EqState = serde_json::from_str(json).unwrap();
        assert!(s.enabled && s.protect && !s.normalize);
        assert_eq!(s.bands[0].kind, BandType::Lowshelf);
        let back = serde_json::to_value(&s).unwrap();
        assert_eq!(back["bands"][0]["type"], "lowshelf");
    }
}
