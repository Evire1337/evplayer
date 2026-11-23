//! The plain rules of a session (src/jam/protocol.ts), kept apart from the network so they can
//! be tested: codes, where the host's song is right now, and how much of the queue fits in one
//! message. The messages themselves are the page's JSON as it is, so both builds understand
//! each other.

use serde_json::{json, Value};

use crate::model::Track;

const CODE_CHARS: &[u8] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
const CODE_LENGTH: usize = 8;
pub const LINK_PREFIX: &str = "evplayer://jam/";
/// the public list of open sessions: a room everyone knows the key of
pub const LOBBY: &str = "EVPLAYER-LOBBY";
/// only a jump this big is worth a seek (small drift is not heard; a seek is)
pub const DRIFT_MS: f64 = 1500.0;
/// one message is at most this big (the encryption limit is 64 KB, before base64)
const MAX_MESSAGE: usize = 48_000;
/// at most this many people in one session
pub const MAX_PEOPLE: usize = 16;
pub const COLORS: [&str; 10] = ["#6ff0c4", "#ff9a3c", "#3b82f6", "#8b5cf6", "#ec4899", "#10b981", "#06b6d4", "#ef4444", "#f59e0b", "#a855f7"];
pub const REACTIONS: [&str; 8] = ["🔥", "❤️", "👏", "🎉", "🚀", "😭", "⚡", "🎧"];

/// a code that is easy to read out (no 0/O, 1/I): JAM-7K9P-2QXZ
pub fn new_code() -> String {
    let code: String = (0..CODE_LENGTH).map(|_| CODE_CHARS[rand::random::<usize>() % CODE_CHARS.len()] as char).collect();
    format!("JAM-{}-{}", &code[..4], &code[4..])
}

/// a typed or pasted code, or an invite link, as a code ("" when it is not one)
pub fn normalize_code(input: &str) -> String {
    let t = input.trim();
    let t = if t.len() >= LINK_PREFIX.len() && t[..LINK_PREFIX.len()].eq_ignore_ascii_case(LINK_PREFIX) { &t[LINK_PREFIX.len()..] } else if t.to_lowercase().starts_with("evplayer://jam") { &t[14..] } else { t };
    let raw = t.split('~').next().unwrap_or("");
    let chars: String = raw.to_uppercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let chars = chars.strip_prefix("JAM").map(str::to_string).unwrap_or(chars);
    if chars.chars().count() == CODE_LENGTH {
        format!("JAM-{}-{}", &chars[..4], &chars[4..])
    } else {
        String::new()
    }
}

/// what comes after "~" in an invite from Discord: the host's pass past "ask first"
pub fn ticket_of(input: &str) -> Option<String> {
    input.split('~').nth(1).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

pub fn invite_link(code: &str) -> String {
    format!("{LINK_PREFIX}{code}")
}

/// where the host's song is now, by their report from `at`
pub fn position_now(now: &Value, clock_ms: f64, duration_ms: f64) -> f64 {
    let pos = now["positionMs"].as_f64().unwrap_or(0.0);
    let at = now["at"].as_f64().unwrap_or(clock_ms);
    let ms = if now["playing"].as_bool().unwrap_or(false) { pos + (clock_ms - at).max(0.0) } else { pos };
    (if duration_ms > 0.0 { ms.min(duration_ms) } else { ms }).max(0.0)
}

/// a part of the queue around `index`: a few songs back and the next ones, as many as fit in
/// a message (a long queue is never sent whole)
pub fn queue_window(queue: &[Track], index: usize, rev: u64, ahead: usize) -> Value {
    let mut n = ahead;
    loop {
        let offset = index.saturating_sub(n.min(5));
        let end = (index + 1 + n).min(queue.len());
        let part = json!({ "rev": rev, "offset": offset, "tracks": &queue[offset.min(end)..end] });
        if n <= 1 || part.to_string().len() <= MAX_MESSAGE {
            return part;
        }
        n /= 2;
    }
}

/// a list of songs a guest asks to play, cut to what fits in a message: (tracks, start)
pub fn fit_tracks(tracks: &[Track], start: usize) -> (Vec<Track>, usize) {
    let part = queue_window(tracks, start, 0, 40);
    let offset = part["offset"].as_u64().unwrap_or(0) as usize;
    let list: Vec<Track> = serde_json::from_value(part["tracks"].clone()).unwrap_or_default();
    (list, start - offset)
}

/// names count as the same regardless of case and spaces around
pub fn same_name(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

const KINDS: [&str; 16] = ["join", "here", "leave", "ask", "resync", "state", "refuse", "wait", "kick", "moved", "end", "note", "chat", "react", "open", "gone"];

/// one of the session's messages (anything else read off a relay is dropped)
pub fn is_message(v: &Value) -> bool {
    v.is_object() && v["t"].as_str().is_some_and(|t| KINDS.contains(&t))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(i: usize, extra: usize) -> Track {
        Track { id: format!("sc:{i}"), title: format!("Song {i}{}", "x".repeat(extra)), duration_ms: 200_000.0, ..Default::default() }
    }

    #[test]
    fn makes_readable_codes_and_reads_them_back_from_anything_typed_or_pasted() {
        let code = new_code();
        assert!(regex::Regex::new(r"^JAM-[2-9A-HJ-NP-Z]{4}-[2-9A-HJ-NP-Z]{4}$").unwrap().is_match(&code));
        assert_eq!(normalize_code(&code), code);
        assert_eq!(normalize_code(" jam - 7k9p 2qxz "), "JAM-7K9P-2QXZ");
        assert_eq!(normalize_code("7k9p2qxz"), "JAM-7K9P-2QXZ");
        assert_eq!(normalize_code(&invite_link("JAM-7K9P-2QXZ")), "JAM-7K9P-2QXZ");
        assert_eq!(normalize_code("JAM-7K9P-2Q"), "");
        assert_eq!(normalize_code(""), "");
    }

    #[test]
    fn reads_the_host_s_pass_from_a_discord_invite() {
        assert_eq!(normalize_code("JAM-7K9P-2QXZ~a1b2"), "JAM-7K9P-2QXZ");
        assert_eq!(ticket_of("JAM-7K9P-2QXZ~a1b2").as_deref(), Some("a1b2"));
        assert_eq!(ticket_of("JAM-7K9P-2QXZ"), None);
    }

    #[test]
    fn the_position_moves_on_while_playing_and_stays_within_the_song() {
        let now = json!({ "index": 0, "trackId": "sc:1", "playing": true, "positionMs": 10_000, "at": 1_000 });
        assert_eq!(position_now(&now, 4_000.0, 0.0), 13_000.0);
        let mut paused = now.clone();
        paused["playing"] = false.into();
        assert_eq!(position_now(&paused, 4_000.0, 0.0), 10_000.0);
        assert_eq!(position_now(&now, 999_000.0, 60_000.0), 60_000.0);
        assert_eq!(position_now(&now, 0.0, 0.0), 10_000.0);
    }

    #[test]
    fn sends_a_window_of_the_queue_that_fits_in_one_message() {
        let queue: Vec<Track> = (0..500).map(|i| track(i, 0)).collect();
        let w = queue_window(&queue, 100, 7, 60);
        assert_eq!(w["rev"], 7);
        assert_eq!(w["offset"], 95);
        assert_eq!(w["tracks"][0]["id"], "sc:95");
        assert_eq!(w["tracks"][5]["id"], "sc:100");
        assert!(w["tracks"].as_array().unwrap().len() < 80);
        assert_eq!(queue_window(&queue, 2, 0, 60)["offset"], 0);
        let big: Vec<Track> = (0..200).map(|i| track(i, 2000)).collect();
        let w = queue_window(&big, 50, 0, 60);
        assert!(w.to_string().len() <= 48_000);
        let off = w["offset"].as_u64().unwrap() as usize;
        assert_eq!(w["tracks"][50 - off]["id"], "sc:50");
        let (list, start) = fit_tracks(&queue, 300);
        assert_eq!(list[start].id, "sc:300");
    }

    #[test]
    fn takes_only_the_session_s_messages_and_compares_names_loosely() {
        assert!(is_message(&json!({ "t": "chat", "id": "1", "text": "hi" })));
        assert!(!is_message(&json!({ "t": "hack" })));
        assert!(!is_message(&Value::Null));
        assert!(same_name(" Вася ", "вася"));
        assert!(!same_name("Вася", "Вася2"));
    }
}
