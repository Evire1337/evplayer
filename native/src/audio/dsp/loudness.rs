//! Volume levelling (src/equalizer/loudness.ts): songs are mastered at wildly different
//! levels; this follows how loud the current one is (a slow moving average of its power, so
//! single drum hits do not pump) and brings it toward one target. The estimate is remembered
//! per song (evplayer.loudness.v1), so a song heard before starts at its level.

pub const TARGET_DB: f64 = -18.0;
pub const MAX_BOOST_DB: f64 = 8.0;
pub const MAX_CUT_DB: f64 = -12.0;
const SILENCE_DB: f64 = -55.0;
pub const MAX_REMEMBERED: usize = 3000;

#[derive(Clone, Debug, Default)]
pub struct Leveler {
    avg_power: Option<f64>,
    heard_ms: f64,
}

impl Leveler {
    pub fn new(initial_db: Option<f64>) -> Self {
        match initial_db {
            Some(db) => Self { avg_power: Some(10f64.powf(db / 10.0)), heard_ms: 30_000.0 },
            None => Self::default(),
        }
    }

    /// one measurement (the mean square of the samples) taken `dt_ms` after the last
    pub fn update(&mut self, power: f64, dt_ms: f64) {
        let db = 10.0 * power.max(1e-12).log10();
        if db < SILENCE_DB {
            return; // pauses and silent intros say nothing about the level
        }
        // fast during the first seconds of an unknown song, slowly afterwards
        let tau = if self.heard_ms < 6000.0 { 1500.0 } else { 8000.0 };
        let alpha = 1.0 - (-dt_ms / tau).exp();
        self.avg_power = Some(match self.avg_power {
            None => power,
            Some(a) => a + alpha * (power - a),
        });
        self.heard_ms += dt_ms;
    }

    pub fn loudness_db(&self) -> Option<f64> {
        self.avg_power.map(|p| 10.0 * p.log10())
    }

    pub fn gain_db(&self, target: f64) -> f64 {
        match self.loudness_db() {
            None => 0.0,
            Some(l) => (target - l).clamp(MAX_CUT_DB, MAX_BOOST_DB),
        }
    }

    pub fn settled(&self) -> bool {
        self.heard_ms >= 20_000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(l: &mut Leveler, db: f64, ms: f64) {
        let mut t = 0.0;
        while t < ms {
            l.update(10f64.powf(db / 10.0), 150.0);
            t += 150.0;
        }
    }

    #[test]
    fn a_loud_song_down_a_quiet_one_up() {
        let mut loud = Leveler::new(None);
        feed(&mut loud, -8.0, 10_000.0);
        assert!((loud.gain_db(TARGET_DB) + 10.0).abs() < 1.0);
        let mut quiet = Leveler::new(None);
        feed(&mut quiet, -24.0, 10_000.0);
        assert!((quiet.gain_db(TARGET_DB) - 6.0).abs() < 1.0);
    }

    #[test]
    fn ignores_silence_caps_the_boost_and_remembers() {
        let mut l = Leveler::new(None);
        feed(&mut l, -80.0, 5000.0);
        assert_eq!(l.gain_db(TARGET_DB), 0.0);
        feed(&mut l, -40.0, 30_000.0);
        assert_eq!(l.gain_db(TARGET_DB), MAX_BOOST_DB);
        let r = Leveler::new(Some(-10.0));
        assert!(r.settled());
        assert!((r.gain_db(TARGET_DB) + 8.0).abs() < 1e-9);
    }
}
