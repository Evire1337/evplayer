//! "Подключить Яндекс Музыку" (src/services/yandex/account.ts): a profile by login. Its public
//! playlists and "Мне нравится" are carried over; every song is found on YouTube Music or
//! SoundCloud. No password: Яндекс Музыка shows what the profile made public.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::import::Named;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Account {
    pub login: String,
    pub name: String,
    /// public playlists when connected
    #[serde(default)]
    pub playlists: usize,
}

pub fn account() -> Option<Account> {
    crate::state::get().extra("yandexAccount").and_then(|v| serde_json::from_value(v).ok())
}

fn save(a: Option<&Account>) {
    crate::state::get().set_extra("yandexAccount", a.and_then(|a| serde_json::to_value(a).ok()).unwrap_or(Value::Null));
}

pub struct Profile {
    pub login: String,
    pub name: String,
    /// (id, title, cover)
    pub playlists: Vec<(String, String, Option<String>)>,
}

fn enc(s: &str) -> String {
    percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string()
}

pub fn profile(login: &str) -> Result<Profile, String> {
    let clean = login.trim().trim_start_matches('@').to_string();
    let list = crate::import::yandex_get(&format!("/users/{}/playlists/list", enc(&clean)))?;
    let list = list.as_array().cloned().unwrap_or_default();
    let name = list.first().and_then(|p| p.pointer("/owner/name")).and_then(Value::as_str).unwrap_or(&clean).to_string();
    let playlists = list
        .iter()
        .filter_map(|p| {
            let id = p.get("kind").and_then(Value::as_u64)?.to_string();
            let title = p.get("title").and_then(Value::as_str).unwrap_or("Плейлист").to_string();
            let cover = p.pointer("/cover/uri").and_then(Value::as_str).map(|u| format!("https://{}", u.replace("%%", "400x400")));
            Some((id, title, cover))
        })
        .collect();
    Ok(Profile { login: clean, name, playlists })
}

/// "Подключить": a login or a link music.yandex.ru/users/…
pub fn connect(input: &str) -> Result<(), String> {
    let value = input.trim();
    let login = regex::Regex::new(r"music\.yandex\.[a-z]+/users/([^/?#]+)").unwrap().captures(value).map(|m| percent_encoding::percent_decode_str(&m[1]).decode_utf8_lossy().to_string()).unwrap_or_else(|| value.to_string());
    let p = profile(&login)?;
    save(Some(&Account { login: p.login, name: p.name, playlists: p.playlists.len() }));
    Ok(())
}

pub fn disconnect() {
    save(None);
}

/// "Мне нравится" of a profile, if it is public
pub fn likes(login: &str) -> Result<Vec<Named>, String> {
    let likes = crate::import::yandex_get(&format!("/users/{}/likes/tracks", enc(login)))?;
    let ids: Vec<String> = likes
        .pointer("/library/tracks")
        .and_then(Value::as_array)
        .map(|l| {
            l.iter()
                .filter_map(|t| {
                    let id = t.get("id").map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()))?;
                    Some(match t.get("albumId") {
                        Some(a) if !a.is_null() => format!("{id}:{}", a.as_str().map(str::to_string).unwrap_or_else(|| a.to_string())),
                        _ => id,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let mut out = Vec::new();
    for chunk in ids.chunks(200) {
        let res = crate::services::http::client().post("https://api.music.yandex.net/tracks").header("content-type", "application/x-www-form-urlencoded").body(format!("track-ids={}", enc(&chunk.join(",")))).send().map_err(|e| e.to_string())?;
        let body: Value = res.json().unwrap_or(Value::Null);
        out.extend(body.get("result").and_then(Value::as_array).map(|l| l.iter().filter_map(crate::import::yandex_named).collect::<Vec<_>>()).unwrap_or_default());
    }
    Ok(out)
}

/// the songs of one of the profile's playlists
pub fn playlist(login: &str, id: &str) -> Result<Vec<Named>, String> {
    let r = crate::import::yandex_get(&format!("/users/{}/playlists/{id}", enc(login)))?;
    Ok(r.get("tracks").and_then(Value::as_array).map(|l| l.iter().filter_map(|t| t.get("track").and_then(crate::import::yandex_named)).collect()).unwrap_or_default())
}
