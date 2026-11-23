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
    ("bass", "Басы", [6.0, 2.5, 0.0, 0.0, 0.0, 0.0]),
    ("vocal", "Голос", [-1.0, -2.0, 0.0, 3.0, 2.5, 0.5]),
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

#[cfg(test)]
mod tests {
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
