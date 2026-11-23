//! Telling the outside what plays: Windows' media flyout and the media keys of the keyboard
//! (SystemMediaTransportControls, the same thing the browser's Media Session drove), the
//! status in Discord (discord.rs, shared with the Tauri build) and the tray's tooltip.
//!
//! One thread does it, woken when the player changes; it sleeps otherwise.

use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;

use crate::player::{Player, Status};

struct Wake {
    flag: Mutex<bool>,
    cv: Condvar,
}

static WAKE: Wake = Wake { flag: Mutex::new(false), cv: Condvar::new() };
static DISCORD: OnceLock<crate::discord::Discord> = OnceLock::new();

/// the player changed: tell the outside soon
pub fn changed() {
    *WAKE.flag.lock().unwrap() = true;
    WAKE.cv.notify_one();
}

/// "Подключено как …" for Настройки → Discord
pub fn status() -> String {
    let Some(d) = DISCORD.get() else { return crate::i18n::t("Выключено") };
    let s = d.status();
    match (s.status, s.user) {
        ("connected", Some(u)) => format!("{} {}", crate::i18n::t("Подключено как"), u.global_name.unwrap_or(u.username)),
        ("connected", None) => crate::i18n::t("Подключено"),
        (_, _) if s.error.is_some() => crate::i18n::t("Discord не запущен"),
        _ => crate::i18n::t("Подключаюсь…"),
    }
}

#[derive(PartialEq, Clone)]
struct Shown {
    id: String,
    status: Status,
    /// the position at the last update, to notice a seek
    at: i64,
}

pub fn start(player: Arc<Player>, hwnd: isize) {
    #[cfg(windows)]
    let smtc = smtc::Smtc::new(hwnd);
    #[cfg(not(windows))]
    let _ = hwnd;
    let _ = DISCORD.set(crate::discord::Discord::new());
    std::thread::Builder::new()
        .name("presence".into())
        .spawn(move || {
            let mut last: Option<Shown> = None;
            loop {
                {
                    let mut f = WAKE.flag.lock().unwrap();
                    while !*f {
                        f = WAKE.cv.wait(f).unwrap();
                    }
                    *f = false;
                }
                // a burst of changes (a song loading) is one update
                std::thread::sleep(Duration::from_millis(120));
                *WAKE.flag.lock().unwrap() = false;
                let view = player.view();
                let cur = view.current().cloned();
                let shown = Shown { id: cur.as_ref().map(|t| t.id.clone()).unwrap_or_default(), status: view.status, at: (view.position_ms / 2000.0) as i64 };
                if last.as_ref() == Some(&shown) {
                    continue;
                }
                let song_changed = last.as_ref().map(|l| l.id != shown.id).unwrap_or(true);
                last = Some(shown);
                let playing = matches!(view.status, Status::Playing | Status::Loading);
                crate::system::set_playing(playing);
                if song_changed {
                    let tip = cur.as_ref().map(|t| format!("{} — {}", t.title, t.artist_line())).unwrap_or_else(|| "EvPlayer".into());
                    crate::system::set_tooltip(&tip);
                }
                #[cfg(windows)]
                if let Some(s) = &smtc {
                    s.update(cur.as_ref(), playing, song_changed);
                }
                discord(&view);
            }
        })
        .ok();
}

fn discord(view: &crate::player::View) {
    let Some(d) = DISCORD.get() else { return };
    let s = crate::state::get().settings();
    if !s.discord_enabled {
        d.update(None);
        return;
    }
    d.set_app_id(Some(s.discord_app_id()));
    let Some(t) = view.current() else {
        d.update(None);
        return;
    };
    let paused = !matches!(view.status, Status::Playing | Status::Loading);
    if paused && !s.discord_show_paused {
        d.update(None);
        return;
    }
    let now = crate::store::state::now_ms();
    let started = now - view.position_ms;
    // the listening session: its size ("2 из 16") and the "Слушать вместе" button, which takes a
    // friend in through a page (Discord takes only https links); nothing for a closed session.
    // Discord's own "Ask to join" is not used: next to it Discord shows no link buttons at all.
    let (party, join_url) = match crate::jam::engine::get().map(|j| j.view()).filter(|v| v.status == crate::jam::engine::JamStatus::Live) {
        Some(v) => (
            Some(crate::discord::Party { id: format!("evp:{}", v.code), size: v.people.len() as u32, max: crate::jam::protocol::MAX_PEOPLE as u32, join: None }),
            (v.access != crate::jam::engine::Access::Closed).then(|| format!("https://evire1337.github.io/evplayer/join/?code={}", percent_encoding::utf8_percent_encode(&v.code, percent_encoding::NON_ALPHANUMERIC))),
        ),
        None => (None, None),
    };
    let state = crate::discord::PresenceState {
        title: t.title.clone(),
        artist: t.artist_line(),
        artwork: if s.discord_show_artwork { t.artwork.clone().filter(|a| a.starts_with("https://")) } else { None },
        url: if s.discord_show_button { t.permalink.clone() } else { None },
        started_at: (s.discord_show_timeline && !paused).then_some(started),
        ends_at: (s.discord_show_timeline && !paused && view.duration_ms > 0.0).then_some(started + view.duration_ms),
        paused: Some(paused),
        status_shows: Some(match s.discord_status_shows {
            crate::store::state::StatusShows::App => "app",
            crate::store::state::StatusShows::Artist => "artist",
            crate::store::state::StatusShows::Song => "song",
        }.into()),
        position_ms: Some(view.position_ms),
        duration_ms: Some(view.duration_ms),
        party,
        join_url,
    };
    d.update(Some(state));
}

pub fn quit() {
    if let Some(d) = DISCORD.get() {
        d.quit();
    }
}

#[cfg(windows)]
mod smtc {
    use windows::core::{Interface, HSTRING};
    use windows::Foundation::{TypedEventHandler, Uri};
    use windows::Media::{MediaPlaybackStatus, MediaPlaybackType, SystemMediaTransportControls, SystemMediaTransportControlsButton, SystemMediaTransportControlsButtonPressedEventArgs};
    use windows::Storage::Streams::RandomAccessStreamReference;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::WinRT::ISystemMediaTransportControlsInterop;

    use crate::app::actions::Action;
    use crate::events::{self, Event};

    pub struct Smtc {
        c: SystemMediaTransportControls,
    }

    // SAFETY: SystemMediaTransportControls is an agile WinRT object, usable from any thread
    unsafe impl Send for Smtc {}

    impl Smtc {
        pub fn new(hwnd: isize) -> Option<Self> {
            let interop = windows::core::factory::<SystemMediaTransportControls, ISystemMediaTransportControlsInterop>().ok()?;
            // SAFETY: the main window's handle, alive while the program runs
            let c: SystemMediaTransportControls = unsafe { interop.GetForWindow(HWND(hwnd as _)) }.ok()?;
            let _ = c.SetIsEnabled(true);
            let _ = c.SetIsPlayEnabled(true);
            let _ = c.SetIsPauseEnabled(true);
            let _ = c.SetIsNextEnabled(true);
            let _ = c.SetIsPreviousEnabled(true);
            let handler = TypedEventHandler::<SystemMediaTransportControls, SystemMediaTransportControlsButtonPressedEventArgs>::new(|_, args| {
                if let Some(a) = args.as_ref() {
                    let action = match a.Button()? {
                        SystemMediaTransportControlsButton::Play | SystemMediaTransportControlsButton::Pause => Some(Action::PlayPause),
                        SystemMediaTransportControlsButton::Next => Some(Action::Next),
                        SystemMediaTransportControlsButton::Previous => Some(Action::Prev),
                        _ => None,
                    };
                    if let Some(a) = action {
                        events::send(Event::Action(a));
                    }
                }
                Ok(())
            });
            let _ = c.ButtonPressed(&handler);
            let _ = c.cast::<windows::core::IInspectable>();
            Some(Self { c })
        }

        pub fn update(&self, track: Option<&crate::model::Track>, playing: bool, song_changed: bool) {
            let _ = self.c.SetPlaybackStatus(match (track, playing) {
                (None, _) => MediaPlaybackStatus::Stopped,
                (_, true) => MediaPlaybackStatus::Playing,
                (_, false) => MediaPlaybackStatus::Paused,
            });
            if !song_changed {
                return;
            }
            let Ok(d) = self.c.DisplayUpdater() else { return };
            let _ = d.ClearAll();
            if let Some(t) = track {
                let _ = d.SetType(MediaPlaybackType::Music);
                if let Ok(m) = d.MusicProperties() {
                    let _ = m.SetTitle(&HSTRING::from(t.title.as_str()));
                    let _ = m.SetArtist(&HSTRING::from(t.artist_line().as_str()));
                }
                if let Some(art) = t.artwork.as_deref().filter(|a| a.starts_with("https://")) {
                    if let Ok(uri) = Uri::CreateUri(&HSTRING::from(art)) {
                        if let Ok(r) = RandomAccessStreamReference::CreateFromUri(&uri) {
                            let _ = d.SetThumbnail(&r);
                        }
                    }
                }
            }
            let _ = d.Update();
        }
    }
}
