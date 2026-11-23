//! The country of the connection, and keeping SoundCloud out of reach from the countries the
//! person chose (Настройки → Сервисы; off by default), as src/services/region.ts. SoundCloud
//! decides from the address which country the listener is in and marks songs "not available";
//! the app asks a neutral service (through the same route, a VPN included) and does not talk to
//! SoundCloud from those countries: songs then play from their YouTube Music copy.

use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const COUNTRY_URL: &str = "https://www.cloudflare.com/cdn-cgi/trace";
/// the answer is trusted this long (a VPN may be switched on or off any time)
const FRESH: Duration = Duration::from_secs(30);
/// a failed lookup is not repeated for this long
const FAILED: Duration = Duration::from_secs(3);

struct Region {
    country: Option<String>,
    at: Option<Instant>,
    failed_at: Option<Instant>,
}

static REGION: Mutex<Region> = Mutex::new(Region { country: None, at: None, failed_at: None });

/// "loc=PL" from the service's plain-text answer
pub fn parse_country(trace: &str) -> Option<String> {
    trace.lines().find_map(|l| l.strip_prefix("loc=")).filter(|c| c.len() == 2 && c.chars().all(|x| x.is_ascii_uppercase())).map(str::to_string)
}

pub fn normalize_country(input: &str) -> Option<String> {
    let c = input.trim().to_uppercase();
    (c.len() == 2 && c.chars().all(|x| x.is_ascii_uppercase())).then_some(c)
}

/// which country the connection is in now (asked again when the last answer is old)
pub fn lookup_country(force: bool) -> Option<String> {
    {
        let r = REGION.lock().unwrap();
        if !force {
            if let (Some(c), Some(at)) = (&r.country, r.at) {
                if at.elapsed() < FRESH {
                    return Some(c.clone());
                }
            }
            if r.country.is_none() && r.failed_at.is_some_and(|f| f.elapsed() < FAILED) {
                return None;
            }
        }
    }
    let found = super::http::get_text(COUNTRY_URL).ok().and_then(|t| parse_country(&t));
    let mut r = REGION.lock().unwrap();
    match &found {
        Some(c) => {
            r.country = Some(c.clone());
            r.at = Some(Instant::now());
        }
        None => {
            r.country = None;
            r.failed_at = Some(Instant::now());
        }
    }
    found
}

pub fn known_country() -> Option<String> {
    REGION.lock().unwrap().country.clone()
}

/// whether SoundCloud may be asked from this country, by the settings
pub fn allowed(country: Option<&str>, guard: bool, countries: &[String]) -> bool {
    // the country is unknown: nothing is held back (requests fail on their own offline)
    !guard || country.is_none_or(|c| !countries.iter().any(|x| x == c))
}

/// before every request to SoundCloud
pub fn guard_soundcloud() -> Result<(), String> {
    let (guard, countries) = {
        let s = crate::state::get().settings.read().unwrap();
        (s.region_guard, s.region_countries.clone())
    };
    if !guard || countries.is_empty() {
        return Ok(());
    }
    let country = lookup_country(false);
    if allowed(country.as_deref(), guard, &countries) {
        Ok(())
    } else {
        Err(format!("SoundCloud не трогаю с адреса страны {}: включите VPN другой страны или отключите защиту в Настройки → Сервисы", country.unwrap_or_default()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_country_and_decides() {
        assert_eq!(parse_country("fl=1\nloc=PL\ntls=TLSv1.3").as_deref(), Some("PL"));
        assert_eq!(parse_country("loc=pl"), None);
        assert_eq!(normalize_country(" ru ").as_deref(), Some("RU"));
        let ru = vec!["RU".to_string()];
        assert!(!allowed(Some("RU"), true, &ru));
        assert!(allowed(Some("DE"), true, &ru));
        assert!(allowed(None, true, &ru));
        assert!(allowed(Some("RU"), false, &ru));
    }
}
