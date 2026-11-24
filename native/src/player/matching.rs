//! "Is this the same recording": the rules of src/player/match.ts, word for word. A song from
//! one service that will not play is played from its copy on the other; another artist's song
//! of the same name, a parody, a remix or a sped-up version is not that copy. Change with care
//! and with the tests below (the same cases as match.test.ts).

use std::collections::HashSet;
use std::sync::OnceLock;

use regex::Regex;

use crate::model::Track;

fn re(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).expect("pattern"))
}

static NOISE: OnceLock<Regex> = OnceLock::new();
static REUPLOAD: OnceLock<Regex> = OnceLock::new();
static FEAT: OnceLock<Regex> = OnceLock::new();
static NON_WORD: OnceLock<Regex> = OnceLock::new();
static SPLIT: OnceLock<Regex> = OnceLock::new();
static BRACKETS: OnceLock<Regex> = OnceLock::new();

pub fn simplify(value: &str) -> String {
    let v = value.to_lowercase();
    let v = re(&NOISE, r"(?i)\((?:official|lyrics?|audio|video|music video|hd|hq|4k|visuali[sz]er|клип|текст|премьера)[^)]*\)|\[(?:[^\]]*)\]|official (?:music )?video|lyric video|official audio").replace_all(&v, " ");
    let v = re(&REUPLOAD, r"(?i)\(?\s*(?:all )?credits? (?:go(?:es)? )?to [^)]*\)?|\bnot my (?:song|music|track)\b|\bno copyright(?: infringement)?(?: intended)?\b|\bi do not own (?:this|the) (?:song|music)\b").replace_all(&v, " ");
    let v = re(&FEAT, r"(?i)\b(?:feat|ft|prod)\.?\s.*$").replace_all(&v, " ");
    let v = re(&NON_WORD, r"[^\p{L}\p{N}]+").replace_all(&v, " ");
    v.trim().to_string()
}

fn words(value: &str) -> HashSet<String> {
    simplify(value).split(' ').filter(|w| w.chars().count() > 1 || w.chars().any(|c| c.is_ascii_digit())).map(str::to_string).collect()
}

fn overlap(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let shared = a.iter().filter(|w| b.contains(*w)).count();
    shared as f64 / a.len().min(b.len()) as f64
}

/// "Artist - Title" uploads put the artist into the title: split it back out
pub fn split_title(track: &Track) -> (String, String) {
    if let Some(m) = re(&SPLIT, r"^(.+?)\s+[-–—]\s+(.+)$").captures(&track.title) {
        return (m[2].to_string(), m[1].to_string());
    }
    (track.title.clone(), track.artists.first().map(|a| a.name.clone()).unwrap_or_default())
}

/// markers of another version of a song: (tag, the words, words that may not come right before)
const VERSION_TAGS: &[(&str, &str, &[&str])] = &[
    ("remix", "remix|rmx|ремикс", &[]),
    ("cover", "cover|кавер", &[]),
    ("sped", r"sped|speed ?up|ускор\p{L}*", &[]),
    ("slowed", r"slowed|замедл\p{L}*", &[]),
    ("reverb", "reverb", &[]),
    ("nightcore", "nightcore", &[]),
    ("live", r"live|концерт\p{L}*", &[]),
    ("instrumental", "instrumental|минус|минусовка", &[]),
    ("karaoke", "karaoke|караоке", &[]),
    ("8d", "8d", &[]),
    // "radio edit" / "clean edit" are the same song
    ("edit", "edit", &["radio ", "clean ", "explicit "]),
    ("bootleg", "bootleg", &[]),
    ("flip", "flip", &[]),
    ("mashup", "mash ?up|мэшап", &[]),
    ("phonk", "phonk|фонк", &[]),
    ("vip", "vip", &[]),
    ("rework", "rework|remake", &[]),
    ("bass", "bass ?boost(?:ed)?", &[]),
    ("acoustic", r"acoustic|акустик\p{L}*", &[]),
    // "Club Mix", "Extended Mix", but not "Original Mix"
    ("mix", "mix", &["original "]),
    // sound-alikes and fan takes
    ("parody", "parody|parodie|пародия|пародии", &[]),
    ("tribute", "tribute|трибьют", &[]),
    ("imitation", "in the style of|made famous by|originally performed by|type beat|в стиле", &[]),
    ("lofi", "lo-?fi|лофи", &[]),
    ("tiktok", "tik ?tok|тик ?ток", &[]),
    ("fanmade", "fan ?made", &[]),
];

fn tag_patterns() -> &'static Vec<(&'static str, Regex, &'static [&'static str])> {
    static P: OnceLock<Vec<(&'static str, Regex, &'static [&'static str])>> = OnceLock::new();
    P.get_or_init(|| VERSION_TAGS.iter().map(|(tag, p, not_after)| (*tag, Regex::new(&format!("(?:{p})")).expect("tag"), *not_after)).collect())
}

fn is_word_char(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric())
}

pub fn version_tags(title: &str) -> HashSet<&'static str> {
    let t = title.to_lowercase().replace('ё', "е");
    let mut out = HashSet::new();
    for (tag, re, not_after) in tag_patterns() {
        // a whole word in any alphabet (what \b does for Latin only)
        let found = re.find_iter(&t).any(|m| {
            let before = t[..m.start()].chars().next_back();
            let after = t[m.end()..].chars().next();
            !is_word_char(before) && !is_word_char(after) && !not_after.iter().any(|p| t[..m.start()].ends_with(p))
        });
        if found {
            out.insert(*tag);
        }
    }
    out
}

/// what each version is called on screen, in the order the tags are listed
const VERSION_LABELS: &[(&str, &str)] = &[
    ("remix", "ремикс"), ("cover", "кавер"), ("sped", "speed up"), ("slowed", "slowed"), ("reverb", "reverb"), ("nightcore", "nightcore"),
    ("live", "live"), ("instrumental", "минус"), ("karaoke", "караоке"), ("8d", "8D"), ("edit", "edit"), ("bootleg", "bootleg"),
    ("flip", "flip"), ("mashup", "мэшап"), ("phonk", "phonk"), ("vip", "VIP"), ("rework", "rework"), ("bass", "bass boost"),
    ("acoustic", "акустика"), ("mix", "mix"), ("parody", "пародия"), ("tribute", "трибьют"), ("imitation", "в стиле"),
    ("lofi", "lo-fi"), ("tiktok", "tiktok"), ("fanmade", "фан-версия"),
];

/// The marks a song's title carries that make it not the original — "ремикс", "speed up",
/// "кавер" — in a fixed order, for the label next to its name. Kept per title: rows ask every
/// time they are drawn.
pub fn version_labels(title: &str) -> Vec<&'static str> {
    thread_local! {
        static SEEN: std::cell::RefCell<std::collections::HashMap<String, Vec<&'static str>>> = std::cell::RefCell::new(Default::default());
    }
    SEEN.with(|seen| {
        if let Some(v) = seen.borrow().get(title) {
            return v.clone();
        }
        let tags = version_tags(title);
        let v: Vec<&'static str> = VERSION_LABELS.iter().filter(|(tag, _)| tags.contains(tag)).map(|(_, label)| *label).collect();
        let mut s = seen.borrow_mut();
        // a long session sees many titles: the memory starts over rather than grows
        if s.len() > 2000 {
            s.clear();
        }
        s.insert(title.to_string(), v.clone());
        v
    })
}

fn artist_names(t: &Track) -> String {
    t.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(" ")
}

/// like overlap, but a name inside a longer one counts ("mikeey" in "mikeeysmind")
fn name_overlap(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let shared = a
        .iter()
        .filter(|w| b.iter().any(|x| x == *w || (x.chars().count().min(w.chars().count()) >= 4 && (x.starts_with(w.as_str()) || w.starts_with(x.as_str())))))
        .count();
    shared as f64 / a.len().min(b.len()) as f64
}

/// 0..1: does the candidate name the wanted artist, as its artist or in its title
pub fn artist_score(wanted: &Track, candidate: &Track) -> f64 {
    name_overlap(&words(&split_title(wanted).1), &words(&format!("{} {}", artist_names(candidate), candidate.title)))
}

/// the title without what is in brackets: "VVV (HE'S BACK)" is "vvv"
fn main_title(t: &Track) -> String {
    simplify(&re(&BRACKETS, r"\([^)]*\)|\[[^\]]*\]").replace_all(&split_title(t).0, " "))
}

fn same_version(wanted: &Track, candidate: &Track) -> bool {
    version_tags(&wanted.title) == version_tags(&candidate.title) && !extra_bracket(wanted, candidate) && !extra_bracket(candidate, wanted)
}

static BRACKET_PARTS: OnceLock<Regex> = OnceLock::new();

/// what a bracket may say without being another version: how the release is named, who is on
/// it, its year
const PLAIN_BRACKET_WORDS: &[&str] = &[
    "remaster", "remastered", "radio", "edit", "version", "original", "explicit", "clean", "single", "album", "deluxe", "bonus", "track",
    "mono", "stereo", "official", "audio", "video", "music", "lyric", "lyrics", "visualizer", "visualiser", "hd", "hq", "4k", "from", "the",
    "оригинал", "версия", "официальный", "клип", "текст", "премьера", "сингл",
    // a re-upload's credit line: «(all credits to …)», «(no copyright)»
    "all", "credit", "credits", "go", "goes", "to", "no", "copyright", "not", "my", "song", "own",
];

/// The candidate's brackets say something the wanted song does not: «Школа не нужна (mylancore)»,
/// «(Пират)», «(дуэт)» — most often another person's version. Who is featured or produced it,
/// the year and the release's own words ("Remastered", "Radio Edit") do not count.
pub fn extra_bracket(wanted: &Track, candidate: &Track) -> bool {
    let known = words(&format!("{} {}", wanted.title, artist_names(wanted)));
    let candidate_names = words(&artist_names(candidate));
    re(&BRACKET_PARTS, r"\(([^)]*)\)|\[([^\]]*)\]").captures_iter(&candidate.title).any(|c| {
        let inside = c.get(1).or_else(|| c.get(2)).map(|m| m.as_str()).unwrap_or("").to_lowercase();
        let lead = inside.trim_start();
        // "(feat. X)", "(prod. by X)", "(with X)", "(при уч. X)": who is on it, not a version
        if ["feat", "ft", "prod", "with", "при уч", "при участии", "w/"].iter().any(|p| lead.starts_with(p)) {
            return false;
        }
        let flat = re(&NON_WORD, r"[^\p{L}\p{N}]+").replace_all(&inside, " ").to_string();
        flat.split(' ').filter(|w| w.chars().count() > 1).any(|w| {
            !w.chars().all(|ch| ch.is_ascii_digit()) && !PLAIN_BRACKET_WORDS.contains(&w) && !known.contains(w) && !candidate_names.contains(w)
        })
    })
}

/// a recording this long is not a song but a mix, a stream or a video: never a song's
/// "original" unless the song itself is that long
const SONG_LENGTH_LIMIT_MS: f64 = 7.0 * 60_000.0;

/// The artist's own release of the song: the same artist, the same title (brackets aside) and
/// not another version. It may be much longer (a SoundCloud upload is often a part of it).
pub fn is_original(wanted: &Track, candidate: &Track) -> bool {
    let title = main_title(wanted);
    if title.is_empty() || title != main_title(candidate) || version_tags(&wanted.title) != version_tags(&candidate.title) || extra_bracket(wanted, candidate) {
        return false;
    }
    let diff = if wanted.duration_ms > 0.0 && candidate.duration_ms > 0.0 { (wanted.duration_ms - candidate.duration_ms).abs() } else { 0.0 };
    // the wanted song's own bracket ("(mylancore)") is another version, unless the wanted one is
    // plainly a part of the candidate (a snippet titled "VVV (HE'S BACK)" of "VVV")
    let a_part = wanted.duration_ms > 0.0 && wanted.duration_ms < candidate.duration_ms * 0.6;
    if extra_bracket(candidate, wanted) && !a_part {
        return false;
    }
    if diff > 15000.0 && wanted.duration_ms > candidate.duration_ms * 0.6 {
        return false;
    }
    // much longer is fine for a part of the song (a snippet's full version), not for an
    // eleven-minute video standing in for a two-minute song
    if diff > 15000.0 && candidate.duration_ms > wanted.duration_ms && candidate.duration_ms > SONG_LENGTH_LIMIT_MS {
        return false;
    }
    name_overlap(&words(&split_title(wanted).1), &words(&artist_names(candidate))) == 1.0
}

/// 0..1: all of the wanted title is in the candidate's, and the candidate's has little besides
fn title_score(wanted: &Track, candidate: &Track) -> f64 {
    let (wt, wa) = split_title(wanted);
    let (ct, ca) = split_title(candidate);
    let wanted_title = words(&wt);
    let found = overlap(&wanted_title, &words(&format!("{ct} {ca}")));
    let credited = words(&format!("{ca} {}", artist_names(candidate)));
    let named: HashSet<String> = words(&ct).into_iter().filter(|x| !credited.contains(x)).collect();
    let wanted_artist = words(&wa);
    let extra = if named.is_empty() { 1.0 } else { named.iter().filter(|x| wanted_title.contains(*x) || wanted_artist.contains(*x)).count() as f64 / named.len() as f64 };
    if found > 0.0 && extra > 0.0 {
        2.0 * found * extra / (found + extra)
    } else {
        0.0
    }
}

/// 0..1: how sure we are that `candidate` is the same recording as `wanted`
pub fn match_score(wanted: &Track, candidate: &Track) -> f64 {
    let artist = artist_score(wanted, candidate);
    let mut score = title_score(wanted, candidate) * 0.6 + artist * 0.3;
    if wanted.duration_ms > 0.0 && candidate.duration_ms > 0.0 {
        // a remix or an edit is usually a different length: that counts for a lot
        let diff = (wanted.duration_ms - candidate.duration_ms).abs();
        score += if diff < 4000.0 {
            0.1
        } else if diff < 8000.0 {
            0.05
        } else if diff < 15000.0 {
            0.0
        } else if diff < 30000.0 {
            -0.2
        } else {
            -0.5
        };
    }
    let a = version_tags(&wanted.title);
    let b = version_tags(&candidate.title);
    for tag in a.union(&b) {
        if a.contains(tag) != b.contains(tag) {
            score -= 0.35;
        }
    }
    // a bracket naming someone else ("(mylancore)") counts as another version too
    if extra_bracket(wanted, candidate) || extra_bracket(candidate, wanted) {
        score -= 0.35;
    }
    score
}

/// Good enough to play in place of the song: a close match of about the same length.
/// `any_artist`: the candidates are a catalogue (YouTube Music songs) and the wanted artist may
/// be just an uploader's name.
pub fn is_same_recording(wanted: &Track, candidate: &Track, any_artist: bool) -> bool {
    let diff = if wanted.duration_ms > 0.0 && candidate.duration_ms > 0.0 { (wanted.duration_ms - candidate.duration_ms).abs() } else { f64::INFINITY };
    if is_original(wanted, candidate) {
        return true;
    }
    if diff.is_finite() && diff > 15000.0 {
        return false;
    }
    if match_score(wanted, candidate) >= 0.8 {
        return true;
    }
    any_artist && diff < 3000.0 && title_score(wanted, candidate) == 1.0 && same_version(wanted, candidate)
}

/// scores this close are the same quality of match: only then does the uncensored one decide
const SCORE_TIE: f64 = 0.05;

/// copies of one song, best first: the artist's own release, then the closest; between about
/// equally close ones, the uncensored version
pub fn copy_order(wanted: &Track, a: &Track, b: &Track) -> std::cmp::Ordering {
    let original = is_original(wanted, b) as i32 - is_original(wanted, a) as i32;
    if original != 0 {
        return original.cmp(&0);
    }
    let gap = match_score(wanted, b) - match_score(wanted, a);
    if gap.abs() >= SCORE_TIE {
        return gap.partial_cmp(&0.0).unwrap_or(std::cmp::Ordering::Equal);
    }
    let marks = b.explicit as i32 - a.explicit as i32;
    if marks != 0 {
        return marks.cmp(&0);
    }
    gap.partial_cmp(&0.0).unwrap_or(std::cmp::Ordering::Equal)
}

/// the best candidate over `threshold`; `require_artist`: it must name the wanted artist;
/// `playable`: previews are skipped
pub fn best_match<'a>(wanted: &Track, candidates: &'a [Track], threshold: f64, require_artist: bool, playable: bool) -> Option<&'a Track> {
    let mut best: Option<&Track> = None;
    let mut best_score = threshold;
    for c in candidates {
        if playable && c.preview {
            continue;
        }
        if require_artist && !wanted.artists.is_empty() && artist_score(wanted, c) < 0.5 {
            continue;
        }
        let score = match_score(wanted, c);
        if score > best_score || (score == best_score && !(best.is_some_and(|b| b.explicit) && !c.explicit)) {
            best = Some(c);
            best_score = score;
        }
    }
    best
}

pub fn search_query_for(track: &Track) -> String {
    let (title, artist) = split_title(track);
    format!("{} {}", simplify(&artist), simplify(&title)).trim().to_string()
}

/// the same song uploaded twice counts once
pub fn song_key(t: &Track) -> String {
    let (title, artist) = split_title(t);
    format!("{}|{}", simplify(&artist), simplify(&title))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Artist, Source};

    fn tr(title: &str, artist: &str, duration: f64, id: &str) -> Track {
        Track { id: id.into(), source: Source::Soundcloud, title: title.into(), artists: vec![Artist { id: artist.into(), name: artist.into() }], duration_ms: duration, ..Default::default() }
    }
    fn t(title: &str, artist: &str, duration: f64) -> Track {
        tr(title, artist, duration, title)
    }
    fn yt(mut t: Track) -> Track {
        t.source = Source::Youtube;
        t
    }

    #[test]
    fn cleans_titles() {
        assert_eq!(simplify("Get Lucky (Official Video) feat. Pharrell"), "get lucky");
    }

    #[test]
    fn prefers_the_same_song_with_a_similar_length() {
        let w = yt(t("Daft Punk - Get Lucky (Official Video)", "Daft Punk", 248000.0));
        let good = tr("Get Lucky", "Daft Punk", 247000.0, "good");
        let remix = tr("Get Lucky (Remix)", "Daft Punk", 300000.0, "remix");
        let other = tr("Lose Yourself to Dance", "Daft Punk", 250000.0, "other");
        assert!(match_score(&w, &good) > match_score(&w, &remix));
        assert_eq!(best_match(&w, &[other.clone(), remix, good], 0.72, false, false).unwrap().id, "good");
        assert!(best_match(&w, &[other], 0.72, false, false).is_none());
    }

    #[test]
    fn names_the_versions_a_title_marks() {
        assert_eq!(version_labels("He's back [Phonk Remix]"), ["ремикс", "phonk"]);
        assert_eq!(version_labels("Супермаркет (speed up)"), ["speed up"]);
        assert!(version_labels("Song (Original Mix)").is_empty());
        assert!(version_labels("Школа не нужна").is_empty());
    }

    #[test]
    fn a_long_video_is_not_the_original_of_a_short_song() {
        // «Школа не нужна» (1:51) was played from an 11:43 video with the same name
        let w = t("Школа не нужна", "ПОШЛАЯ МОЛЛИ", 111000.0);
        assert!(!is_original(&w, &yt(t("Школа не нужна", "Пошлая Молли", 703000.0))));
        assert!(!is_same_recording(&w, &yt(t("Школа не нужна", "Пошлая Молли", 703000.0)), true));
        // a one-minute part still gets its whole song
        assert!(is_original(&t("Школа не нужна", "ПОШЛАЯ МОЛЛИ", 60000.0), &yt(t("Школа не нужна", "Пошлая Молли", 200000.0))));
    }

    #[test]
    fn a_bracket_naming_someone_else_is_another_version() {
        let w = t("Школа не нужна", "uglystephan", 124000.0);
        assert!(!is_same_recording(&w, &yt(t("Школа не нужна (mylancore)", "uglystephan", 126000.0)), true));
        assert!(!is_same_recording(&w, &yt(t("Школа не нужна (Пират)", "serrpir", 124000.0)), true));
        // and the other way: a remix wanted is not the original
        assert!(!is_same_recording(&t("uglystephan - Школа не нужна (mylancore)", "extradiller", 126000.0), &yt(t("Школа не нужна", "uglystephan", 124000.0)), true));
        // who is on it, the year and the release's words are the same song
        assert!(is_same_recording(&t("Мишка", "Пошлая Молли", 156000.0), &yt(t("Мишка (feat. KATERINA)", "Пошлая Молли", 159000.0)), true));
        assert!(is_same_recording(&w, &yt(t("Школа не нужна (Remastered 2021)", "uglystephan", 124000.0)), true));
        assert!(is_same_recording(&w, &yt(t("Школа не нужна (Official Audio)", "uglystephan", 124000.0)), true));
    }

    #[test]
    fn does_not_take_a_longer_title_by_the_same_artist() {
        let w = yt(t("Love", "Kendrick Lamar", 213000.0));
        assert!(!is_same_recording(&w, &t("Love Me Harder", "Kendrick Lamar", 214000.0), false));
        assert!(is_same_recording(&w, &t("LOVE.", "Kendrick Lamar", 213500.0), false));
        assert!(is_same_recording(&w, &t("Kendrick Lamar - Love (Official Audio)", "kendricklamar", 213000.0), false));
    }

    #[test]
    fn takes_a_copy_under_another_name_when_title_and_length_agree() {
        let sc = t("VVV (HE'S BACK)", "mikeeysmind", 80000.0);
        assert!(is_same_recording(&sc, &yt(t("VVV (HE'S BACK)", "Mikeey", 81000.0)), true));
        assert!(is_same_recording(&sc, &yt(t("VVV (HE'S BACK)", "Someone Else", 81000.0)), true));
        assert!(!is_same_recording(&sc, &yt(t("VVV (HE'S BACK)", "Someone Else", 81000.0)), false));
        assert!(!is_same_recording(&sc, &yt(t("VVV (HE'S BACK)", "Someone Else", 95000.0)), false));
        assert!(!is_same_recording(&sc, &yt(t("VVV (HE'S BACK) (Sped Up)", "Mikeey", 81000.0)), false));
        assert!(is_same_recording(&sc, &yt(t("VVV HE'S BACK (ALL CREDITS TO mikeeysmind) not my song", "CYBERMASK", 81000.0)), true));
        let original = yt(t("VVV", "mikeeysmind", 228000.0));
        assert!(is_same_recording(&sc, &original, false));
        assert!(is_original(&sc, &original));
        let mut other = original.clone();
        other.artists = vec![Artist { id: "x".into(), name: "WizNation".into() }];
        assert!(!is_original(&sc, &other));
        assert!(!is_original(&t("Love Story", "Artist", 200000.0), &t("Love", "Artist", 100000.0)));
        let mut remix = original.clone();
        remix.title = "VVV (Remix)".into();
        assert!(!is_original(&sc, &remix));
    }

    #[test]
    fn not_a_parody_or_a_stranger_s_song() {
        let wanted = tr("Not Like Us", "Kendrick Lamar", 274000.0, "spotify");
        let original = tr("Not Like Us", "Kendrick Lamar", 275000.0, "original");
        let stranger = tr("Not Like Us", "DJ Random", 276000.0, "stranger");
        let parody = tr("Kendrick Lamar - Not Like Us (Parody)", "xX_fan_Xx", 274000.0, "parody");
        let style = tr("Not Like Us (In the style of Kendrick Lamar)", "Karaoke Hits", 274000.0, "style");
        assert!(version_tags("Not Like Us (Parody)").contains("parody"));
        assert!(version_tags("Песня (пародия)").contains("parody"));
        assert!(is_same_recording(&wanted, &original, false));
        for other in [&stranger, &parody, &style] {
            assert!(!is_same_recording(&wanted, other, false), "{}", other.id);
        }
        let all = [stranger.clone(), parody, style, original];
        assert_eq!(best_match(&wanted, &all, 0.65, true, false).unwrap().id, "original");
        assert!(best_match(&wanted, &[stranger], 0.65, true, false).is_none());
    }

    #[test]
    fn does_not_import_a_preview() {
        let wanted = t("Blinding Lights", "The Weeknd", 200000.0);
        let mut preview = tr("Blinding Lights", "The Weeknd", 200000.0, "preview");
        preview.preview = true;
        let full = tr("Blinding Lights", "The Weeknd", 201000.0, "full");
        assert_eq!(best_match(&wanted, &[preview.clone(), full], 0.65, false, true).unwrap().id, "full");
        assert!(best_match(&wanted, &[preview], 0.65, false, true).is_none());
    }

    #[test]
    fn builds_a_search_query() {
        assert_eq!(search_query_for(&t("Daft Punk - Get Lucky (Official Video)", "convar", 1.0)), "daft punk get lucky");
    }

    #[test]
    fn sees_version_tags_in_brackets() {
        let mut tags: Vec<_> = version_tags("Song [Phonk Remix]").into_iter().collect();
        tags.sort();
        assert_eq!(tags, vec!["phonk", "remix"]);
        assert!(!version_tags("Song (Radio Edit)").contains("edit"));
        assert!(!version_tags("Song (Original Mix)").contains("mix"));
        assert!(version_tags("Песня (ускоренная версия)").contains("sped"));
    }

    #[test]
    fn does_not_play_a_remix_in_place_of_the_original() {
        let w = yt(t("He's back", "OUENZA", 184000.0));
        assert!(!is_same_recording(&w, &t("OUENZA - He's back [Phonk Remix]", "someone", 186000.0), false));
        assert!(!is_same_recording(&w, &t("He's back", "OUENZA", 212000.0), false));
        assert!(is_same_recording(&w, &t("He's Back", "OUENZA", 185000.0), false));
    }

    #[test]
    fn the_uncensored_copy_between_equal_ones_and_never_over_a_better_one() {
        let wanted = tr("Money Trees", "Kendrick Lamar", 386000.0, "want");
        let clean = tr("Money Trees", "Kendrick Lamar", 386000.0, "clean");
        let mut explicit = tr("Money Trees", "Kendrick Lamar", 386000.0, "explicit");
        explicit.explicit = true;
        let mut remix = tr("Money Trees (Remix)", "DJ Someone", 330000.0, "remix");
        remix.explicit = true;
        let mut v = vec![clean.clone(), explicit.clone()];
        v.sort_by(|a, b| copy_order(&wanted, a, b));
        assert_eq!(v[0].id, "explicit");
        let mut v = vec![explicit.clone(), clean.clone()];
        v.sort_by(|a, b| copy_order(&wanted, a, b));
        assert_eq!(v[0].id, "explicit");
        let mut v = vec![remix, clean.clone()];
        v.sort_by(|a, b| copy_order(&wanted, a, b));
        assert_eq!(v[0].id, "clean");
        let target = tr("Money Trees", "Kendrick Lamar", 386000.0, "name:Money Trees");
        assert_eq!(best_match(&target, &[explicit.clone(), clean.clone()], 0.65, true, false).unwrap().id, "explicit");
        assert_eq!(best_match(&target, &[clean, explicit], 0.65, true, false).unwrap().id, "explicit");
    }
}
