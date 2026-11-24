//! Lyrics from LRCLIB (lrclib.net), timed line by line where the record fits the song, with
//! the rules of src/services/lyrics/lrclib.ts and lrc.ts: a record is trusted for its times
//! only when its length is within 3 seconds of the song's (other versions run ahead or behind).

use std::sync::Mutex;

use serde_json::Value;

use crate::model::Track;

const API: &str = "https://lrclib.net/api";
const SYNC_MAX_DIFF_S: f64 = 3.0;
const TEXT_MAX_DIFF_S: f64 = 20.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    /// -1 for a line without a time
    pub time_ms: f64,
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct Lyrics {
    pub synced: bool,
    pub lines: Vec<Line>,
}

pub fn normalize_title(v: &str) -> String {
    use regex::Regex;
    use std::sync::OnceLock;
    static R: OnceLock<[Regex; 4]> = OnceLock::new();
    let r = R.get_or_init(|| {
        [
            Regex::new(r"[(\[{][^)\]}]*[)\]}]").unwrap(),
            Regex::new(r"\b(feat|ft|featuring|prod)\b\.?.*$").unwrap(),
            Regex::new(r"\b(official|video|audio|lyrics?|hd|hq|remaster(ed)?|клип)\b").unwrap(),
            Regex::new(r"[^\p{L}\p{N}]+").unwrap(),
        ]
    });
    let s = v.to_lowercase().replace('ё', "е");
    let s = r[0].replace_all(&s, " ");
    let s = r[1].replace_all(&s, " ");
    let s = r[2].replace_all(&s, " ");
    r[3].replace_all(&s, " ").trim().to_string()
}

fn readings(t: &Track) -> Vec<(String, String)> {
    let artist = t.artists.first().map(|a| a.name.as_str()).unwrap_or("");
    let artist = regex::Regex::new(r"(?i)\s*-\s*topic$").map(|r| r.replace(artist, "").trim().to_string()).unwrap_or_default();
    let mut out = vec![(t.title.clone(), artist)];
    if let Some(m) = regex::Regex::new(r"^(.+?)\s+[-–—]\s+(.+)$").ok().and_then(|r| r.captures(&t.title)) {
        out.push((m[2].to_string(), m[1].to_string()));
    }
    out
}

fn diff(r: &Value, duration_ms: f64) -> Option<f64> {
    let d = r.get("duration").and_then(Value::as_f64)?;
    (duration_ms > 0.0 && d > 0.0).then(|| (duration_ms / 1000.0 - d).abs())
}

fn synced_fits(r: &Value, duration_ms: f64) -> bool {
    r.get("syncedLyrics").and_then(Value::as_str).is_some_and(|s| !s.is_empty()) && diff(r, duration_ms).is_none_or(|d| d <= SYNC_MAX_DIFF_S)
}

fn score(r: &Value, title: &str, duration_ms: f64) -> f64 {
    let has = |k: &str| r.get(k).and_then(Value::as_str).is_some_and(|s| !s.is_empty());
    if r.get("instrumental").and_then(Value::as_bool) == Some(true) || !(has("syncedLyrics") || has("plainLyrics")) {
        return -1.0;
    }
    let want = normalize_title(title);
    let got = normalize_title(r.get("trackName").and_then(Value::as_str).unwrap_or(""));
    if want.is_empty() || got.is_empty() {
        return -1.0;
    }
    let mut s = if want == got {
        60.0
    } else if want.contains(&got) || got.contains(&want) {
        35.0
    } else {
        return -1.0;
    };
    if let Some(d) = diff(r, duration_ms) {
        if d > TEXT_MAX_DIFF_S {
            return -1.0;
        }
        s += (30.0 - d * 6.0).max(0.0);
    }
    if synced_fits(r, duration_ms) {
        s += 10.0;
    }
    s
}

/// "[01:23.45] words" → timed lines, sorted; untimed text gives one line per row
pub fn parse_lrc(source: &str) -> Lyrics {
    let stamp = regex::Regex::new(r"\[(\d{1,3}):(\d{1,2})(?:[.:](\d{1,3}))?\]").unwrap();
    let tag = regex::Regex::new(r"(?i)^\[[a-z]+:.*\]$").unwrap();
    let mut lines = Vec::new();
    let mut timed = false;
    for raw in source.lines() {
        let stamps: Vec<_> = stamp.captures_iter(raw).collect();
        let text = stamp.replace_all(raw, "").trim().to_string();
        if stamps.is_empty() {
            if !text.is_empty() && !tag.is_match(raw.trim()) {
                lines.push(Line { time_ms: -1.0, text });
            }
            continue;
        }
        timed = true;
        for m in stamps {
            let frac = m.get(3).map(|f| format!("{:0<3}", f.as_str()).parse::<f64>().unwrap_or(0.0)).unwrap_or(0.0);
            let t = m[1].parse::<f64>().unwrap_or(0.0) * 60_000.0 + m[2].parse::<f64>().unwrap_or(0.0) * 1000.0 + frac;
            lines.push(Line { time_ms: t, text: text.clone() });
        }
    }
    if !timed {
        return Lyrics { synced: false, lines };
    }
    lines.retain(|l| l.time_ms >= 0.0);
    lines.sort_by(|a, b| a.time_ms.partial_cmp(&b.time_ms).unwrap_or(std::cmp::Ordering::Equal));
    Lyrics { synced: true, lines }
}

/// the line being sung at `pos`, or None before the first
pub fn active_line(lines: &[Line], pos: f64) -> Option<usize> {
    let i = lines.partition_point(|l| l.time_ms <= pos);
    i.checked_sub(1)
}

/// the lyrics looked up last, oldest first, as many as the page keeps (CACHE_LIMIT in useLyrics.ts):
/// the program lives in the tray for hours, so this must not grow without end
static CACHE: Mutex<Vec<(String, Option<Lyrics>)>> = Mutex::new(Vec::new());
const CACHE_LIMIT: usize = 40;

fn search(params: &[(&str, &str)]) -> Result<Vec<Value>, String> {
    let mut url = url::Url::parse(&format!("{API}/search")).unwrap();
    url.query_pairs_mut().extend_pairs(params);
    Ok(super::http::get_json(url.as_str(), &[])?.as_array().cloned().unwrap_or_default())
}

/// The lyrics of a song: Ok(None) when the service has none, Err when it could not be asked
/// (no network): that is said as such and not remembered, so the next look asks again.
pub fn find(track: &Track) -> Result<Option<Lyrics>, String> {
    if let Some(hit) = CACHE.lock().unwrap().iter().find(|c| c.0 == track.id).map(|c| c.1.clone()) {
        return Ok(hit);
    }
    let mut candidates = Vec::new();
    let mut answered = false;
    let mut failure = String::new();
    let mut ask = |params: &[(&str, &str)], into: &mut Vec<Value>| match search(params) {
        Ok(v) => {
            answered = true;
            into.extend(v);
        }
        Err(e) => failure = e,
    };
    let rs = readings(track);
    for (title, artist) in &rs {
        ask(&[("track_name", title), ("artist_name", artist)], &mut candidates);
        if candidates.iter().any(|c| c.get("syncedLyrics").and_then(Value::as_str).is_some()) {
            break;
        }
    }
    if !candidates.iter().any(|c| c.get("syncedLyrics").and_then(Value::as_str).is_some()) {
        let (title, artist) = rs.last().cloned().unwrap_or_default();
        ask(&[("q", &format!("{artist} {}", normalize_title(&title)))], &mut candidates);
    }
    if !answered {
        return Err(failure);
    }
    let mut best: Option<&Value> = None;
    let mut best_score = 0.0;
    for (title, _) in &rs {
        for c in &candidates {
            let s = score(c, title, track.duration_ms);
            if s > best_score {
                best_score = s;
                best = Some(c);
            }
        }
    }
    let found = best.and_then(|r| {
        if synced_fits(r, track.duration_ms) {
            let text = r.get("syncedLyrics")?.as_str()?.trim().to_string();
            return Some(parse_lrc(&text));
        }
        // a version of another length: its words are right, its times are not
        let words = r.get("plainLyrics").and_then(Value::as_str).map(str::to_string).filter(|s| !s.trim().is_empty()).or_else(|| r.get("syncedLyrics").and_then(Value::as_str).map(|s| parse_lrc(s).lines.iter().map(|l| l.text.clone()).collect::<Vec<_>>().join("\n")))?;
        Some(Lyrics { synced: false, lines: words.lines().map(str::trim).filter(|l| !l.is_empty()).map(|l| Line { time_ms: -1.0, text: l.to_string() }).collect() })
    });
    // kept unless "Ответы сервисов" are not to be kept
    if !crate::state::get().settings.read().unwrap().no_service_cache {
        let mut c = CACHE.lock().unwrap();
        c.retain(|x| x.0 != track.id);
        c.push((track.id.clone(), found.clone()));
        if c.len() > CACHE_LIMIT {
            c.remove(0);
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_timed_lines() {
        let l = parse_lrc("[ar:Someone]\n[00:12.30] first\n[00:05.1] zero\n[01:02.345][01:10.00] chorus\n");
        assert!(l.synced);
        assert_eq!(l.lines[0], Line { time_ms: 5100.0, text: "zero".into() });
        assert_eq!(l.lines[1].time_ms, 12300.0);
        assert_eq!(l.lines.len(), 4);
        assert_eq!(active_line(&l.lines, 0.0), None);
        assert_eq!(active_line(&l.lines, 12_500.0), Some(1));
        let plain = parse_lrc("one\ntwo");
        assert!(!plain.synced);
        assert_eq!(plain.lines.len(), 2);
    }

    #[test]
    fn trusts_times_only_for_the_same_length() {
        let r = serde_json::json!({ "trackName": "Song (Remastered)", "duration": 200.0, "syncedLyrics": "[00:01.00] a", "plainLyrics": "a" });
        assert!(synced_fits(&r, 201_000.0));
        assert!(!synced_fits(&r, 210_000.0));
        assert!(score(&r, "Song", 201_000.0) > score(&r, "Song", 215_000.0));
        assert!(score(&r, "Other", 201_000.0) < 0.0);
        assert_eq!(normalize_title("Ёлка (Official Video) feat. X"), "елка");
    }
}
