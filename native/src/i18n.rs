//! The interface language. Texts are written in Russian in the code, as in the Tauri build;
//! English comes from the same dictionary (src/i18n/en.ts and enMore.ts, read from their source
//! when English is chosen, so there is one dictionary for both builds) and the patterns for
//! texts built from parts ("6 треков", "Добавлено в «Любимое»"), as src/i18n/patterns.ts.
//! Names of songs and artists are never translated.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

static ENGLISH: AtomicBool = AtomicBool::new(false);

pub fn is_english() -> bool {
    ENGLISH.load(Ordering::Relaxed)
}

pub fn set_english(on: bool) {
    ENGLISH.store(on, Ordering::Relaxed);
}

/// the language for "как в системе": English unless Windows is set to Russian (or a
/// language close to it)
pub fn system_is_russian() -> bool {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetUserDefaultUILanguage() -> u16;
        }
        // SAFETY: a plain query without arguments
        let lang = unsafe { GetUserDefaultUILanguage() } & 0x3ff;
        // Russian, Ukrainian, Belarusian, Kazakh
        matches!(lang, 0x19 | 0x22 | 0x23 | 0x3f)
    }
    #[cfg(not(windows))]
    {
        true
    }
}

/// `"текст": "text",` pairs of a TypeScript dictionary
pub fn parse_dictionary(source: &str, out: &mut HashMap<String, String>) {
    fn string_at(s: &str) -> Option<(String, &str)> {
        let s = s.trim_start();
        let rest = s.strip_prefix('"')?;
        let mut value = String::new();
        let mut chars = rest.char_indices();
        while let Some((i, c)) = chars.next() {
            match c {
                '\\' => match chars.next()?.1 {
                    'n' => value.push('\n'),
                    't' => value.push('\t'),
                    'u' => {
                        let hex: String = (0..4).filter_map(|_| chars.next().map(|x| x.1)).collect();
                        value.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                    }
                    other => value.push(other),
                },
                '"' => return Some((value, &rest[i + 1..])),
                c => value.push(c),
            }
        }
        None
    }
    for line in source.lines() {
        let Some((key, rest)) = string_at(line) else { continue };
        let Some(rest) = rest.trim_start().strip_prefix(':') else { continue };
        let Some((value, _)) = string_at(rest) else { continue };
        out.insert(key, value);
    }
}

fn dictionary() -> &'static HashMap<String, String> {
    static D: OnceLock<HashMap<String, String>> = OnceLock::new();
    D.get_or_init(|| {
        let mut d = HashMap::new();
        parse_dictionary(include_str!("../../src/i18n/en.ts"), &mut d);
        parse_dictionary(include_str!("../../src/i18n/enMore.ts"), &mut d);
        parse_dictionary(EXTRA, &mut d);
        d
    })
}

/// texts only the native build has
const EXTRA: &str = r#"
  "Отрисовка": "Drawing",
  "Видеокарта (Direct3D 11)": "Video card (Direct3D 11)",
  "Процессор": "Processor",
  "Звуковая карта": "Sound card",
  "Память программы": "Program memory",
  "Обложек в памяти": "Covers in memory",
  "Нативная версия": "Native version",
"#;

fn count(n: &str, one: &str, many: &str) -> String {
    let v: f64 = n.chars().filter(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(0.0);
    format!("{n} {}", if v == 1.0 { one } else { many })
}

/// the English of a text built from parts, when it is one
fn pattern(text: &str) -> Option<String> {
    use regex::Regex;
    static P: OnceLock<Vec<(Regex, fn(&regex::Captures) -> String)>> = OnceLock::new();
    let list = P.get_or_init(|| {
        let p = |r: &str, f: fn(&regex::Captures) -> String| (Regex::new(r).expect("pattern"), f);
        vec![
            p(r"^(\d[\d\s\u{a0}]*) трек(?:а|ов)?$", |m| count(&m[1], "track", "tracks")),
            p(r"^(\d[\d\s\u{a0}]*) прослушивани(?:е|я|й)$", |m| count(&m[1], "play", "plays")),
            p(r"^(\d[\d\s\u{a0}]*) слушател(?:ь|я|ей)$", |m| count(&m[1], "listener", "listeners")),
            p(r"^([\d.,]+\S?) подписчик(?:а|ов)?$", |m| format!("{} followers", &m[1])),
            p(r"^(\d+) с$", |m| format!("{} s", &m[1])),
            p(r"^(\d+) мин$", |m| format!("{} min", &m[1])),
            p(r"^(\d+) ч (\d+) мин$", |m| format!("{} h {} min", &m[1], &m[2])),
            p(r"^([\d.,]+) МБ$", |m| format!("{} MB", &m[1])),
            p(r"^([\d.,]+) кГц$", |m| format!("{} kHz", &m[1])),
            p(r"^([+−-]?[\d.,]+) дБ$", |m| format!("{} dB", &m[1])),
            p(r"^([+−-]?[\d.,]+) пт$", |m| format!("{} st", &m[1])),
            p(r"^Плейлист · (.+)$", |m| format!("Playlist · {}", t(&m[1]))),
            p(r"^Альбом · (.+)$", |m| format!("Album · {}", &m[1])),
            p(r"^создан (.+)$", |m| format!("created {}", &m[1])),
            p(r"^(.*?)(~?\d+) кбит/с$", |m| format!("{}{} kbps", &m[1], &m[2])),
            p(r"^Похоже на «(.+)»$", |m| format!("Similar to \"{}\"", &m[1])),
            p(r"^Добавлено в «(.+)»$", |m| format!("Added to \"{}\"", &m[1])),
            p(r"^Уже есть в «(.+)»$", |m| format!("Already in \"{}\"", &m[1])),
            p(r"^Плейлист «(.+)» создан$", |m| format!("Playlist \"{}\" created", &m[1])),
            p(r"^Версия (.*)$", |m| format!("Version {}", &m[1])),
            p(r"^Исполнитель · (.+)$", |m| format!("Artist · {}", &m[1])),
            p(r"^Играет (.+)$", |m| format!("Playing {}", &m[1])),
            p(r"^(\d+) из (\d+)$", |m| format!("{} of {}", &m[1], &m[2])),
        ]
    });
    list.iter().find_map(|(re, f)| re.captures(text).map(|m| f(&m)))
}

/// the text in the interface's language
pub fn t(ru: &str) -> String {
    if !is_english() {
        return ru.to_string();
    }
    if let Some(en) = dictionary().get(ru) {
        return en.clone();
    }
    pattern(ru).unwrap_or_else(|| ru.to_string())
}

/// "6 треков", "1 трек", "3 трека" (and English when chosen)
pub fn tracks(n: usize) -> String {
    let word = if n % 10 == 1 && n % 100 != 11 {
        "трек"
    } else if (2..=4).contains(&(n % 10)) && !(12..=14).contains(&(n % 100)) {
        "трека"
    } else {
        "треков"
    };
    t(&format!("{n} {word}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_dictionary_and_the_patterns() {
        let d = dictionary();
        assert_eq!(d.get("Главная").map(String::as_str), Some("Home"));
        assert_eq!(d.get("Открыть «Сейчас играет»").map(String::as_str), Some("Open \"Now playing\""));
        assert!(d.len() > 700, "{}", d.len());
        assert_eq!(pattern("6 треков").as_deref(), Some("6 tracks"));
        assert_eq!(pattern("1 трек").as_deref(), Some("1 track"));
        assert_eq!(pattern("aac 160 кбит/с").as_deref(), Some("aac 160 kbps"));
        assert_eq!(pattern("Добавлено в «Утро»").as_deref(), Some("Added to \"Утро\""));
    }

    #[test]
    fn russian_counts() {
        set_english(false);
        assert_eq!(tracks(1), "1 трек");
        assert_eq!(tracks(3), "3 трека");
        assert_eq!(tracks(11), "11 треков");
        assert_eq!(tracks(22), "22 трека");
    }
}
