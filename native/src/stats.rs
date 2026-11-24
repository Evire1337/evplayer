//! Listening statistics (src/stats/listeningStats.ts): plays and listened time per song per
//! day, kept in evplayer.stats.v1 in the page's own shape, so the Tauri build's history goes on
//! here. They feed "Сегодня прослушано", "Ваш топ-10" and the smart playlists. Loaded the first
//! time something needs them.

use std::collections::BTreeMap;
use std::sync::Mutex;

use chrono::{Datelike, Duration as Days, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::model::Track;

const KEY: &str = "evplayer.stats.v1";
/// days older than this are folded into a running total to keep the store small
const KEEP_DAYS: i64 = 400;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Row {
    pub n: u32,
    pub ms: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    pub track: Track,
    #[serde(default)]
    pub days: BTreeMap<String, Row>,
    #[serde(default)]
    pub old: Row,
    pub first: String,
    pub last: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sms: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Data {
    pub version: u32,
    #[serde(default)]
    pub tracks: BTreeMap<String, Record>,
    #[serde(flatten)]
    pub rest: Map<String, Value>,
}

impl Default for Data {
    fn default() -> Self {
        Self { version: 1, tracks: BTreeMap::new(), rest: Map::new() }
    }
}

static DATA: Mutex<Option<Data>> = Mutex::new(None);

/// answers already worked out, until the next listen changes them (the home page asks every frame)
#[derive(Default)]
struct Memo {
    version: u64,
    tops: std::collections::HashMap<u8, Vec<(Track, u32, f64)>>,
    artists: std::collections::HashMap<u8, Vec<TopArtist>>,
    smart: std::collections::HashMap<String, Vec<Track>>,
    today: Option<f64>,
}

static MEMO: Mutex<Option<Memo>> = Mutex::new(None);
static VERSION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn memo<R>(f: impl FnOnce(&mut Memo) -> R) -> R {
    let v = VERSION.load(std::sync::atomic::Ordering::Relaxed);
    let mut g = MEMO.lock().unwrap();
    let m = g.get_or_insert_with(Memo::default);
    if m.version != v {
        *m = Memo { version: v, ..Default::default() };
    }
    f(m)
}

fn with<R>(f: impl FnOnce(&mut Data) -> R) -> R {
    let mut g = DATA.lock().unwrap();
    let d = g.get_or_insert_with(|| crate::store::kv::read_plain::<Data>(&crate::state::get().kv, KEY).filter(|d| d.version == 1).unwrap_or_default());
    f(d)
}

fn save(d: &Data) {
    crate::store::kv::write_plain(&crate::state::get().kv, KEY, d);
}

pub fn day_key(d: NaiveDate) -> String {
    format!("{:04}-{:02}-{:02}", d.year(), d.month(), d.day())
}

fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// a listen: the same playback session reported again adds only the extra time
pub fn record_in(d: &mut Data, track: &Track, session: &str, played_ms: f64, now: chrono::DateTime<Local>) {
    let iso = now.with_timezone(&chrono::Utc).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let r = d.tracks.entry(track.id.clone()).or_insert_with(|| Record { track: track.clone(), days: BTreeMap::new(), old: Row::default(), first: iso.clone(), last: iso.clone(), sid: None, sms: None });
    r.track = track.clone();
    r.last = iso;
    let row = r.days.entry(day_key(now.date_naive())).or_default();
    let same = r.sid.as_deref() == Some(session);
    if same {
        row.ms += (played_ms - r.sms.unwrap_or(0.0)).max(0.0);
        r.sms = Some(played_ms.max(r.sms.unwrap_or(0.0)));
    } else {
        row.n += 1;
        row.ms += played_ms.max(0.0);
        r.sms = Some(played_ms);
    }
    r.sid = Some(session.to_string());
    // fold old days into the total
    let limit = day_key(now.date_naive() - Days::days(KEEP_DAYS));
    let old: Vec<String> = r.days.keys().filter(|k| k.as_str() < limit.as_str()).cloned().collect();
    for k in old {
        if let Some(x) = r.days.remove(&k) {
            r.old.n += x.n;
            r.old.ms += x.ms;
        }
    }
}

pub fn record(track: &Track, session: &str, played_ms: f64) {
    if crate::state::get().settings.read().unwrap().no_stats || track.id.is_empty() {
        return;
    }
    with(|d| {
        record_in(d, track, session, played_ms, Local::now());
        save(d);
    });
    VERSION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// "Сбросить статистику": every listen forgotten
pub fn clear() {
    with(|d| {
        *d = Data::default();
        save(d);
    });
    VERSION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// how many times a song was played, all time ("Ваши прослушивания" order)
pub fn plays_of(id: &str) -> u32 {
    let today = today();
    with(|d| d.tracks.get(id).map(|r| sum(r, Range::All, today).n).unwrap_or(0))
}

/// One song's statistics (getTrackStats in listeningStats.ts): the week, the month and all
/// time, the first and last listen, plays per day for 30 days and per month for 12 months,
/// oldest first.
#[derive(Clone, Debug, Default)]
pub struct TrackSummary {
    pub week: Row,
    pub month: Row,
    pub all: Row,
    pub first: Option<String>,
    pub last: Option<String>,
    pub daily: Vec<(NaiveDate, Row)>,
    pub monthly: Vec<(NaiveDate, Row)>,
}

pub fn track_summary_in(d: &Data, id: &str, now: NaiveDate) -> TrackSummary {
    let r = d.tracks.get(id);
    let daily: Vec<(NaiveDate, Row)> = (0..30).rev().map(|i| {
        let day = now - Days::days(i);
        (day, r.and_then(|r| r.days.get(&day_key(day)).copied()).unwrap_or_default())
    }).collect();
    let monthly: Vec<(NaiveDate, Row)> = (0..12).rev().map(|i| {
        let total = now.year() * 12 + now.month0() as i32 - i;
        let first = NaiveDate::from_ymd_opt(total.div_euclid(12), total.rem_euclid(12) as u32 + 1, 1).unwrap_or(now);
        let prefix = day_key(first)[..7].to_string();
        let mut row = Row::default();
        if let Some(r) = r {
            for (day, x) in r.days.range(prefix.clone()..) {
                if !day.starts_with(&prefix) {
                    break;
                }
                row.n += x.n;
                row.ms += x.ms;
            }
        }
        (first, row)
    }).collect();
    match r {
        Some(r) => TrackSummary { week: sum(r, Range::Week, now), month: sum(r, Range::Month, now), all: sum(r, Range::All, now), first: Some(r.first.clone()), last: Some(r.last.clone()), daily, monthly },
        None => TrackSummary { daily, monthly, ..Default::default() },
    }
}

pub fn track_summary(id: &str) -> TrackSummary {
    let now = today();
    with(|d| track_summary_in(d, id, now))
}

/// how big the statistics are on disk (Настройки → Данные)
pub fn bytes() -> usize {
    crate::state::get().kv.size(KEY)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Range {
    Week,
    Month,
    All,
}

fn sum(r: &Record, range: Range, now: NaiveDate) -> Row {
    let start = match range {
        Range::All => None,
        Range::Week => Some(day_key(now - Days::days(6))),
        Range::Month => Some(day_key(now - Days::days(29))),
    };
    let mut total = if start.is_some() { Row::default() } else { r.old };
    for (day, row) in &r.days {
        if start.as_ref().is_some_and(|s| day < s) {
            continue;
        }
        total.n += row.n;
        total.ms += row.ms;
    }
    total
}

pub fn top_tracks(range: Range, limit: usize) -> Vec<(Track, u32, f64)> {
    let key = range as u8 * 100 + limit.min(99) as u8;
    if let Some(v) = memo(|m| m.tops.get(&key).cloned()) {
        return v;
    }
    let v = top_tracks_now(range, limit);
    memo(|m| m.tops.insert(key, v.clone()));
    v
}

fn top_tracks_now(range: Range, limit: usize) -> Vec<(Track, u32, f64)> {
    let now = today();
    with(|d| {
        let mut v: Vec<(Track, u32, f64)> = d.tracks.values().map(|r| {
            let t = sum(r, range, now);
            (r.track.clone(), t.n, t.ms)
        }).filter(|x| x.1 > 0).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal)));
        v.truncate(limit);
        v
    })
}

/// an artist of "Ваши топ-исполнители": plays and time summed over their songs, the cover of
/// the first song met (getTopArtists)
#[derive(Clone, Debug, PartialEq)]
pub struct TopArtist {
    pub id: String,
    pub name: String,
    pub plays: u32,
    pub ms: f64,
    pub cover: Option<String>,
}

pub fn top_artists(range: Range, limit: usize) -> Vec<TopArtist> {
    let key = range as u8 * 100 + limit.min(99) as u8;
    if let Some(v) = memo(|m| m.artists.get(&key).cloned()) {
        return v;
    }
    let now = today();
    let v = with(|d| top_artists_in(d.tracks.values().map(|r| (&r.track, sum(r, range, now))), limit));
    memo(|m| m.artists.insert(key, v.clone()));
    v
}

fn top_artists_in<'a>(rows: impl Iterator<Item = (&'a Track, Row)>, limit: usize) -> Vec<TopArtist> {
    let mut by: std::collections::HashMap<String, TopArtist> = std::collections::HashMap::new();
    // the order artists were first met, so ties keep the page's order
    let mut order: Vec<String> = Vec::new();
    for (t, total) in rows {
        if total.n == 0 {
            continue;
        }
        for a in &t.artists {
            let name = a.name.trim();
            if name.is_empty() || name == "Unknown Artist" {
                continue;
            }
            let key = if !a.id.is_empty() && a.id != "unknown" { a.id.clone() } else { format!("name:{}", name.to_lowercase()) };
            let row = by.entry(key.clone()).or_insert_with(|| {
                order.push(key);
                TopArtist { id: a.id.clone(), name: name.to_string(), plays: 0, ms: 0.0, cover: t.artwork.clone() }
            });
            row.plays += total.n;
            row.ms += total.ms;
        }
    }
    let mut v: Vec<TopArtist> = order.into_iter().filter_map(|k| by.remove(&k)).collect();
    v.sort_by(|a, b| b.plays.cmp(&a.plays).then(b.ms.partial_cmp(&a.ms).unwrap_or(std::cmp::Ordering::Equal)));
    v.truncate(limit);
    v
}

/// how long you listened today, all songs together
pub fn listened_today() -> f64 {
    if let Some(v) = memo(|m| m.today) {
        return v;
    }
    let key = day_key(today());
    let v = listened_today_now(&key);
    memo(|m| m.today = Some(v));
    v
}

fn listened_today_now(key: &str) -> f64 {
    with(|d| d.tracks.values().map(|r| r.days.get(key).map(|x| x.ms).unwrap_or(0.0)).sum())
}

/// "3 ч 12 мин", "45 мин", "40 с"
pub fn format_time(ms: f64) -> String {
    let min = (ms / 60_000.0).floor() as u64;
    if min < 1 {
        return format!("{} с", (ms / 1000.0).round().max(0.0));
    }
    let (h, m) = (min / 60, min % 60);
    if h > 0 {
        format!("{h} ч {m} мин")
    } else {
        format!("{m} мин")
    }
}

pub const SMART: [(&str, &str, &str, f32); 4] = [
    ("on_repeat", "На повторе", "То, что вы слушали 3 и больше раз за последний месяц", 160.0),
    ("discoveries", "Новые открытия", "Песни, которые вы впервые услышали за последние две недели", 200.0),
    ("forgotten", "Забытые любимые", "Часто слушали раньше, но не включали полтора месяца", 28.0),
    ("liked_rare", "Лайкнули и забыли", "Понравившиеся, которые вы почти не слушали", 330.0),
];

/// the playlists that fill themselves from your listening
pub fn smart(kind: &str, liked: &[Track], limit: usize) -> Vec<Track> {
    // the likes are part of the answer for "liked_rare": their count marks a change
    let key = format!("{kind}|{limit}|{}", liked.len());
    if let Some(v) = memo(|m| m.smart.get(&key).cloned()) {
        return v;
    }
    let v = smart_now(kind, liked, limit);
    memo(|m| m.smart.insert(key, v.clone()));
    v
}

fn smart_now(kind: &str, liked: &[Track], limit: usize) -> Vec<Track> {
    let now = Local::now();
    let date = now.date_naive();
    let age_days = |iso: &str| chrono::DateTime::parse_from_rfc3339(iso).map(|t| (now.with_timezone(&chrono::Utc) - t.with_timezone(&chrono::Utc)).num_hours() as f64 / 24.0).unwrap_or(f64::MAX);
    with(|d| {
        let mut picked: Vec<(&Record, u32)> = match kind {
            "on_repeat" => d.tracks.values().map(|r| (r, sum(r, Range::Month, date).n)).filter(|x| x.1 >= 3).collect(),
            "discoveries" => d.tracks.values().filter(|r| age_days(&r.first) <= 14.0).map(|r| (r, sum(r, Range::All, date).n)).collect(),
            "forgotten" => d.tracks.values().map(|r| (r, sum(r, Range::All, date).n)).filter(|x| x.1 >= 3 && age_days(&x.0.last) > 45.0).collect(),
            "liked_rare" => return liked.iter().filter(|t| d.tracks.get(&t.id).is_none_or(|r| sum(r, Range::All, date).n <= 1)).take(limit).cloned().collect(),
            _ => vec![],
        };
        picked.sort_by(|a, b| b.1.cmp(&a.1));
        picked.into_iter().take(limit).map(|x| x.0.track.clone()).collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_counts_once_and_adds_only_new_time() {
        let mut d = Data::default();
        let t = Track { id: "sc:1".into(), ..Default::default() };
        let now = Local::now();
        record_in(&mut d, &t, "s1", 5000.0, now);
        record_in(&mut d, &t, "s1", 180_000.0, now);
        record_in(&mut d, &t, "s2", 6000.0, now);
        let r = &d.tracks["sc:1"];
        let row = r.days[&day_key(now.date_naive())];
        assert_eq!(row.n, 2);
        assert_eq!(row.ms, 186_000.0);
        assert_eq!(sum(r, Range::Week, now.date_naive()).n, 2);
    }

    #[test]
    fn old_days_fold_into_the_total_and_the_page_s_data_reads() {
        let json = r#"{"version":1,"tracks":{"sc:1":{"track":{"id":"sc:1","source":"soundcloud","title":"A","artists":[],"durationMs":1},"days":{"2020-01-01":{"n":2,"ms":10}},"old":{"n":1,"ms":5},"first":"2020-01-01T00:00:00.000Z","last":"2020-01-01T00:00:00.000Z","sid":"x","sms":10}},"seeded":true}"#;
        let mut d: Data = serde_json::from_str(json).unwrap();
        let t = d.tracks["sc:1"].track.clone();
        record_in(&mut d, &t, "y", 1000.0, Local::now());
        let r = &d.tracks["sc:1"];
        assert_eq!(r.old, Row { n: 3, ms: 15.0 });
        assert!(!r.days.contains_key("2020-01-01"));
        assert_eq!(serde_json::to_value(&d).unwrap()["seeded"], true);
        assert_eq!(format_time(3_725_000.0), "1 ч 2 мин");
    }

    #[test]
    fn a_song_s_summary_has_its_days_and_months() {
        let mut d = Data::default();
        let t = Track { id: "yt:1".into(), ..Default::default() };
        let now = Local::now();
        let today = now.date_naive();
        record_in(&mut d, &t, "a", 60_000.0, now);
        record_in(&mut d, &t, "b", 30_000.0, now - Days::days(10));
        let s = track_summary_in(&d, "yt:1", today);
        assert_eq!(s.daily.len(), 30);
        assert_eq!(s.daily.last().unwrap().0, today);
        assert_eq!(s.daily.last().unwrap().1.n, 1);
        assert_eq!(s.week.n, 1);
        assert_eq!(s.month.n, 2);
        assert_eq!(s.all.ms, 90_000.0);
        assert_eq!(s.monthly.len(), 12);
        assert_eq!(s.monthly.iter().map(|m| m.1.n).sum::<u32>(), 2);
        assert_eq!(s.monthly.last().unwrap().0.month(), today.month());
        let none = track_summary_in(&d, "yt:2", today);
        assert_eq!(none.all.n, 0);
        assert!(none.first.is_none());
    }
}
