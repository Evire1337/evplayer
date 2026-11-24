//! Tests against the real services (run by hand: `cargo test -- --ignored --nocapture net_`).
//! They search, get a song's sound in each format the services give, and decode its first
//! seconds into memory: nothing is played on the sound card.

#![cfg(test)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::audio::decode::{Block, Decoder, RING_BLOCKS};
use crate::audio::source::Spec;
use crate::services::{soundcloud, youtube};

fn init() {
    std::env::set_var("EVPLAYER_NO_IMPORT", "1");
    let dir = std::env::temp_dir().join("evplayer-native-nettest");
    let _ = std::fs::create_dir_all(&dir);
    crate::state::init(dir);
}

/// decode `spec` until `seconds` of sound are in; the loudest sample and the codec
fn decode_some(spec: Spec, seconds: f64) -> Result<(f32, String, u32), String> {
    let (prod, mut cons) = rtrb::RingBuffer::<Block>::new(RING_BLOCKS);
    let dec = Decoder::start(spec, 0.0, prod);
    let started = Instant::now();
    let mut frames = 0usize;
    let mut peak = 0f32;
    loop {
        if let Some(e) = dec.track.error() {
            return Err(e);
        }
        if started.elapsed() > Duration::from_secs(40) {
            return Err(format!("too slow: {frames} frames in 40 s"));
        }
        match cons.pop() {
            Ok(b) => {
                for s in &b.data[..b.len as usize * 2] {
                    peak = peak.max(s.abs());
                }
                frames += b.len as usize;
                let rate = dec.track.rate.load(Ordering::Relaxed);
                if frames as f64 / rate as f64 >= seconds || b.end {
                    return Ok((peak, dec.track.codec.lock().unwrap().clone(), rate));
                }
            }
            Err(_) => std::thread::sleep(Duration::from_millis(5)),
        }
    }
}

#[test]
#[ignore]
fn net_youtube_plays_aac_and_opus() {
    init();
    let found = youtube::search("daft punk one more time").expect("search");
    assert!(!found.is_empty());
    let id = youtube::video_id_of(&found[0].id).unwrap().to_string();
    let d = youtube::resolve_audio(&id).expect("resolve");
    println!("youtube: itag {} {} via {}", d.itag, d.quality(), &d.user_agent[..20]);
    let started = Instant::now();
    let (peak, codec, rate) = decode_some(Spec::Http { url: d.url.clone(), user_agent: Some(d.user_agent.to_string()) }, 8.0).expect("decode");
    println!("  decoded 8 s of {codec} at {rate} Hz in {:?}, peak {peak:.3}", started.elapsed());
    assert!(peak > 0.01, "silence");
    // Opus in WebM, through libopus
    let o = youtube::resolve_audio_in(&id, &[251, 250, 249]).expect("opus");
    assert_eq!(o.codec, "opus");
    let started = Instant::now();
    let (peak, codec, rate) = decode_some(Spec::Http { url: o.url.clone(), user_agent: Some(o.user_agent.to_string()) }, 8.0).expect("decode opus");
    println!("  decoded 8 s of {codec} (itag {}) at {rate} Hz in {:?}, peak {peak:.3}", o.itag, started.elapsed());
    assert_eq!(rate, 48_000);
    assert!(peak > 0.01, "silence");
}

/// a jump into the middle lands there in both YouTube formats (the seek reads the file's index)
#[test]
#[ignore]
fn net_youtube_seeks_in_both_formats() {
    init();
    let found = youtube::search("radiohead paranoid android").expect("search");
    let id = youtube::video_id_of(&found[0].id).unwrap().to_string();
    for order in [[251u32, 250], [140, 139]] {
        let d = youtube::resolve_audio_in(&id, &order).expect("resolve");
        let (prod, mut cons) = rtrb::RingBuffer::<Block>::new(RING_BLOCKS);
        let dec = Decoder::start(Spec::Http { url: d.url.clone(), user_agent: Some(d.user_agent.to_string()) }, 0.0, prod);
        let started = Instant::now();
        while !dec.track.ready.load(Ordering::Relaxed) {
            assert!(started.elapsed() < Duration::from_secs(30), "no start");
            std::thread::sleep(Duration::from_millis(10));
        }
        let t = Instant::now();
        let epoch = dec.seek(120_000.0);
        let landed = loop {
            assert!(t.elapsed() < Duration::from_secs(30), "no sound after the jump");
            match cons.pop() {
                Ok(b) if b.epoch == epoch && b.len > 0 => break b.pos as f64 / dec.track.rate.load(Ordering::Relaxed) as f64,
                Ok(_) => {}
                Err(_) => std::thread::sleep(Duration::from_millis(5)),
            }
        };
        println!("itag {}: jumped to {landed:.3} s in {:?}", d.itag, t.elapsed());
        assert!((landed - 120.0).abs() < 0.05, "landed at {landed}");
    }
}

#[test]
#[ignore]
fn net_soundcloud_plays_every_format() {
    init();
    let (tracks, _) = soundcloud::search("lofi hip hop").expect("search");
    let mut tried = 0;
    for t in tracks.iter().filter(|t| !t.preview).take(6) {
        let Ok(list) = soundcloud::streams(&t.id) else { continue };
        for tr in list {
            let Ok(s) = soundcloud::stream_url(&tr) else { continue };
            let spec = if s.hls { Spec::Hls { url: s.url.clone() } } else { Spec::Http { url: s.url.clone(), user_agent: None } };
            let started = Instant::now();
            match decode_some(spec, 6.0) {
                Ok((peak, codec, rate)) => {
                    println!("soundcloud {} {} ({}): {codec} {rate} Hz, peak {peak:.3}, {:?}", tr.protocol, tr.mime, s.quality, started.elapsed());
                    assert!(peak > 0.001, "silence");
                    tried += 1;
                }
                Err(e) => println!("soundcloud {} {}: FAILED {e}", tr.protocol, tr.mime),
            }
        }
        if tried >= 3 {
            break;
        }
    }
    assert!(tried > 0, "no SoundCloud format decoded");
}

/// decode `spec` to its very end; the seconds of sound that came out and why it stopped
fn decode_all(spec: Spec) -> (f64, String) {
    let (prod, mut cons) = rtrb::RingBuffer::<Block>::new(RING_BLOCKS);
    let dec = Decoder::start(spec, 0.0, prod);
    let started = Instant::now();
    let mut frames = 0usize;
    loop {
        if let Some(e) = dec.track.error() {
            let rate = dec.track.rate.load(Ordering::Relaxed).max(1);
            return (frames as f64 / rate as f64, format!("error: {e}"));
        }
        if started.elapsed() > Duration::from_secs(240) {
            let rate = dec.track.rate.load(Ordering::Relaxed).max(1);
            return (frames as f64 / rate as f64, "too slow".into());
        }
        match cons.pop() {
            Ok(b) => {
                frames += b.len as usize;
                if b.end {
                    let rate = dec.track.rate.load(Ordering::Relaxed).max(1);
                    return (frames as f64 / rate as f64, "end".into());
                }
            }
            Err(_) => std::thread::sleep(Duration::from_millis(2)),
        }
    }
}

/// the length the player is told for a SoundCloud stream is the whole song's (an HLS stream's
/// first piece once passed for the song: the player faded into the next one after seconds)
#[test]
#[ignore]
fn net_the_length_is_the_whole_song() {
    init();
    std::env::set_var("EVPLAYER_TRACE", "1");
    let (tracks, _) = soundcloud::search("big baby tape big tymers").expect("search");
    let t = tracks.iter().find(|t| !t.preview).expect("a song");
    let mut checked = 0;
    for tr in soundcloud::streams(&t.id).expect("streams") {
        let Ok(s) = soundcloud::stream_url(&tr) else { continue };
        let spec = if s.hls { Spec::Hls { url: s.url.clone() } } else { Spec::Http { url: s.url.clone(), user_agent: None } };
        let (prod, _cons) = rtrb::RingBuffer::<Block>::new(RING_BLOCKS);
        let dec = Decoder::start(spec, 0.0, prod);
        let started = Instant::now();
        while !dec.track.ready.load(Ordering::Relaxed) && started.elapsed() < Duration::from_secs(30) {
            std::thread::sleep(Duration::from_millis(10));
        }
        let got = dec.track.duration_ms().unwrap_or(0.0);
        println!("{} {}: told {:.1} s, the song is {:.1} s", tr.protocol, tr.mime, got / 1000.0, t.duration_ms / 1000.0);
        assert!((got - t.duration_ms).abs() < 3000.0, "told {got} for a {} ms song", t.duration_ms);
        checked += 1;
    }
    assert!(checked > 0);
}

/// Whole songs decode to their end in every format (into memory, nothing is played): a song
/// that stopped after its first piece showed only as "plays 10 seconds and skips".
#[test]
#[ignore]
fn net_whole_songs_play_to_the_end() {
    init();
    let mut bad = Vec::new();
    for q in ["big baby tape big tymers", "kizaru bandana", "lofi hip hop"] {
        let (tracks, _) = soundcloud::search(q).expect("search");
        for t in tracks.iter().filter(|t| !t.preview).take(2) {
            let Ok(list) = soundcloud::streams(&t.id) else { continue };
            for tr in list {
                let Ok(s) = soundcloud::stream_url(&tr) else { continue };
                let spec = if s.hls { Spec::Hls { url: s.url.clone() } } else { Spec::Http { url: s.url.clone(), user_agent: None } };
                let (got, why) = decode_all(spec);
                let want = t.duration_ms / 1000.0;
                println!("soundcloud «{}» {} {}: {got:.1} of {want:.1} s ({why})", t.title, tr.protocol, tr.mime);
                if got < want - 3.0 {
                    bad.push(format!("SC {} {} «{}»: {got:.1}/{want:.1} {why}", tr.protocol, tr.mime, t.title));
                }
            }
        }
    }
    for q in ["big baby tape big tymers", "kizaru"] {
        let found = youtube::search(q).expect("search");
        for t in found.iter().take(2) {
            let id = youtube::video_id_of(&t.id).unwrap().to_string();
            for order in [[251u32, 250], [140, 139]] {
                let Ok(d) = youtube::resolve_audio_in(&id, &order) else { continue };
                let (got, why) = decode_all(Spec::Http { url: d.url.clone(), user_agent: Some(d.user_agent.to_string()) });
                let want = t.duration_ms / 1000.0;
                println!("youtube «{}» itag {}: {got:.1} of {want:.1} s ({why})", t.title, d.itag);
                if got < want - 3.0 {
                    bad.push(format!("YT itag {} «{}»: {got:.1}/{want:.1} {why}", d.itag, t.title));
                }
            }
        }
    }
    assert!(bad.is_empty(), "cut short:\n{}", bad.join("\n"));
}

/// The whole player through the real sound card at volume zero (silence is what reaches the
/// speakers): a YouTube song starts, its position moves with the sound card's clock, a seek
/// lands, "next" goes to the next song, a pause stops the clock.
#[test]
#[ignore]
fn net_player_plays_silently_through_the_sound_card() {
    use crate::player::{Player, Status};
    init();
    let st = crate::state::get();
    let library = crate::library::Library::new(st.data_dir.clone());
    let offline = crate::offline::Offline::new(&st.data_dir);
    let resolver = crate::player::resolve::Resolver::new(library, offline);
    let player = Player::new(resolver, || {});
    // silence first, before anything is loaded
    player.set_volume(0.0);
    let a = youtube::search("daft punk around the world").expect("search");
    let b = youtube::search("daft punk harder better faster").expect("search");
    let c = youtube::search("daft punk digital love").expect("search");
    let queue = vec![a[0].clone(), b[0].clone(), c[0].clone()];
    let wait = |what: &str, f: &dyn Fn() -> bool| {
        let t = Instant::now();
        while !f() {
            assert!(t.elapsed() < Duration::from_secs(40), "waited too long for {what}");
            std::thread::sleep(Duration::from_millis(50));
        }
    };
    player.play_queue(queue.clone(), 0, None);
    wait("playing", &|| player.status() == Status::Playing);
    println!("device: {}", player.device());
    let p0 = player.position_ms();
    std::thread::sleep(Duration::from_millis(1500));
    let p1 = player.position_ms();
    println!("position moved {:.0} ms in 1500 ms", p1 - p0);
    assert!((p1 - p0 - 1500.0).abs() < 250.0, "the clock: {p0} → {p1}");
    player.seek(60_000.0);
    std::thread::sleep(Duration::from_millis(800));
    let p2 = player.position_ms();
    println!("after the seek: {p2:.0}");
    assert!((60_000.0..62_500.0).contains(&p2), "seek: {p2}");
    player.toggle();
    std::thread::sleep(Duration::from_millis(300));
    let paused_at = player.position_ms();
    std::thread::sleep(Duration::from_millis(600));
    assert_eq!(player.status(), Status::Paused);
    assert!((player.position_ms() - paused_at).abs() < 30.0, "a pause stops the clock");
    player.toggle();
    player.next();
    wait("the next song", &|| player.current().map(|t| t.id == queue[1].id).unwrap_or(false) && player.status() == Status::Playing);
    println!("next: {} at {:.0} ms", player.current().unwrap().title, player.position_ms());
    // near the end: the third song is prepared on the other deck and takes over by itself
    let dur = player.view().duration_ms;
    player.seek(dur - 6000.0);
    wait("the song after, gapless", &|| player.current().map(|t| t.id == queue[2].id).unwrap_or(false) && player.status() == Status::Playing);
    println!("took over: {} at {:.0} ms", player.current().unwrap().title, player.position_ms());
    assert!(player.position_ms() < 3000.0);
    assert!(player.view().volume == 0.0);
    player.shutdown();
}

#[test]
#[ignore]
fn net_a_song_is_saved_for_offline_and_then_played_from_disk() {
    init();
    let st = crate::state::get();
    let library = crate::library::Library::new(st.data_dir.clone());
    let offline = crate::offline::Offline::new(&st.data_dir);
    let resolver = crate::player::resolve::Resolver::new(library, offline.clone());
    let d = crate::downloads::Downloads::new(offline.clone(), resolver.clone());
    let t = youtube::search("daft punk one more time").expect("search")[0].clone();
    d.remove(std::slice::from_ref(&t.id));
    assert_eq!(d.add(std::slice::from_ref(&t)), 1);
    let started = Instant::now();
    while !d.is_saved(&t.id) {
        let s = d.status();
        assert!(s.failed.is_empty(), "{:?}", s.failed);
        assert!(started.elapsed() < Duration::from_secs(90), "too slow");
        std::thread::sleep(Duration::from_millis(200));
    }
    let entry = d.saved().into_iter().find(|e| e.id == t.id).unwrap();
    println!("saved {} ({} KB, .{}) in {:?}", t.title, entry.size / 1024, entry.ext, started.elapsed());
    use crate::player::resolve::Resolve;
    let first = resolver.candidates(&t, Arc::new(AtomicBool::new(false))).next().unwrap().unwrap();
    assert!(matches!(first.spec, Spec::File(_)), "the saved copy comes first");
    let (peak, codec, _) = decode_some(first.spec, 3.0).expect("decode the saved file");
    println!("  plays from disk: {codec}, peak {peak:.2}");
}

#[test]
#[ignore]
fn net_imports_read_deezer_and_spotify_and_find_the_songs() {
    init();
    for link in ["https://www.deezer.com/album/302127", "https://open.spotify.com/album/2noRn2Aes5aoNVsU6iWThc"] {
        match crate::import::fetch(link) {
            Ok(f) => {
                println!("{link}: «{}», {} names, first {:?}", f.title, f.named.len(), f.named.first().map(|n| (&n.title, &n.artists)));
                if let Some(first) = f.named.first() {
                    let found = crate::import::match_one(first);
                    println!("  found: {:?}", found.map(|t| format!("{} — {} ({})", t.artist_line(), t.title, t.id)));
                }
            }
            Err(e) => println!("{link}: FAILED {e}"),
        }
    }
}

/// a Spotify playlist longer than the embed page's 100 songs, read whole without signing in
#[test]
#[ignore]
fn net_spotify_reads_a_whole_playlist_without_signing_in() {
    init();
    for id in ["37i9dQZF1DX4UtSsGT1Sbe", "37i9dQZF1DXbTxeAdrVG2l", "37i9dQZF1DWXRqgorJj26U"] {
        match crate::services::spotify::guest_playlist(id, None, &|_, _| {}) {
            Ok((named, total)) => {
                println!("{id}: {} songs of {:?}, first {:?}", named.len(), total, named.first().map(|n| (&n.title, &n.artists)));
                assert!(!named.is_empty());
            }
            Err(e) => println!("{id}: FAILED {e}"),
        }
    }
    let f = crate::import::fetch("https://open.spotify.com/playlist/37i9dQZF1DX4UtSsGT1Sbe").expect("import");
    println!("import: «{}» {} names, truncated {}", f.title, f.named.len(), f.truncated);
}

/// two copies in one session on the real relays: what one says the other reads, signed by the
/// first one's key; a sealed note opens only for its addressee
#[test]
#[ignore]
fn net_jam_two_copies_hear_each_other_through_the_relays() {
    use std::sync::mpsc::channel;
    let (tx, rx) = channel::<(serde_json::Value, String)>();
    let tx = std::sync::Mutex::new(tx);
    let started = Instant::now();
    let b = crate::jam::link::Link::open(Box::new(move |m, from| {
        let _ = tx.lock().unwrap().send((m, from));
    }))
    .expect("relays");
    let a = crate::jam::link::Link::open(Box::new(|_, _| {})).expect("relays");
    println!("connected in {:?}", started.elapsed());
    let code = crate::jam::protocol::new_code();
    a.join(&code);
    b.join(&code);
    std::thread::sleep(Duration::from_secs(2));
    let sent = Instant::now();
    a.send(&serde_json::json!({ "t": "chat", "id": "x1", "text": "привет из нативной версии" }));
    let (msg, from) = rx.recv_timeout(Duration::from_secs(10)).expect("the message came through");
    println!("came in {:?}: {msg} from {}", sent.elapsed(), &from[..8]);
    assert_eq!(msg["text"], "привет из нативной версии");
    assert_eq!(from, a.id());
    let sealed = a.seal("JAM-AAAA-BBBB", &b.id()).unwrap();
    assert_eq!(b.unseal(&sealed, &a.id()).as_deref(), Some("JAM-AAAA-BBBB"));
}

/// A whole session on the real relays with two players at volume zero (silence reaches the
/// sound card): the host plays a song, a guest joins by code, and the guest's player plays
/// the same song at the same place; the guest's "next" goes to the host, and both move on.
#[test]
#[ignore]
fn net_jam_a_guest_follows_the_host() {
    use crate::player::{Player, Status};
    init();
    let st = crate::state::get();
    let make = || {
        let library = crate::library::Library::new(st.data_dir.clone());
        let offline = crate::offline::Offline::new(&st.data_dir);
        let p = Player::new(crate::player::resolve::Resolver::new(library, offline), || {});
        p.set_volume(0.0);
        p
    };
    let (hp, gp) = (make(), make());
    let host = crate::jam::engine::Jam::new(hp.clone());
    let guest = crate::jam::engine::Jam::new(gp.clone());
    host.set_profile(Some("Хост".into()), None);
    guest.set_profile(Some("Гость".into()), None);
    let a = youtube::search("daft punk around the world").expect("search");
    let b = youtube::search("daft punk harder better faster").expect("search");
    hp.play_queue(vec![a[0].clone(), b[0].clone()], 0, None);
    let wait = |what: &str, secs: u64, f: &dyn Fn() -> bool| {
        let t = Instant::now();
        while !f() {
            assert!(t.elapsed() < Duration::from_secs(secs), "waited too long for {what}");
            std::thread::sleep(Duration::from_millis(100));
        }
        println!("{what}: {:?}", t.elapsed());
    };
    wait("the host plays", 40, &|| hp.status() == Status::Playing);
    host.start();
    wait("the session is on", 15, &|| host.view().status == crate::jam::engine::JamStatus::Live);
    let code = host.view().code;
    println!("code {code}");
    guest.join(&code);
    wait("the guest is in", 20, &|| guest.view().status == crate::jam::engine::JamStatus::Live && host.view().people.len() == 2);
    wait("the guest plays the host's song", 40, &|| gp.status() == Status::Playing && gp.current().map(|t| t.id.clone()) == hp.current().map(|t| t.id.clone()));
    std::thread::sleep(Duration::from_secs(5));
    let (h, g) = (hp.position_ms(), gp.position_ms());
    println!("host at {h:.0} ms, guest at {g:.0} ms, apart {:.0} ms", (h - g).abs());
    assert!((h - g).abs() < 1600.0, "the guest drifted: {h} vs {g}");
    // the guest's "next" is a request to the host
    gp.next();
    wait("both on the next song", 40, &|| hp.current().map(|t| t.id.clone()) == Some(b[0].id.clone()) && gp.current().map(|t| t.id.clone()) == Some(b[0].id.clone()) && gp.status() == Status::Playing);
    guest.leave();
    wait("the host sees the guest leave", 15, &|| host.view().people.len() == 1);
    host.leave();
    hp.shutdown();
    gp.shutdown();
}

/// what the updater makes of the real latest.json (nothing is installed)
#[test]
#[ignore]
fn net_the_updater_reads_the_latest_release() {
    init();
    crate::updates::check();
    println!("updates: {:?}", crate::updates::status());
    assert!(!matches!(crate::updates::status(), crate::updates::Status::Idle | crate::updates::Status::Checking));
}

/// the author's real signature of the Tauri installer passes the check (into memory only),
/// and the same bytes changed by one do not
#[test]
#[ignore]
fn net_the_release_signature_checks_out() {
    let latest: serde_json::Value = reqwest::blocking::get("https://github.com/Evire1337/evplayer/releases/latest/download/latest.json").unwrap().json().unwrap();
    let entry = &latest["platforms"]["windows-x86_64"];
    let bytes = reqwest::blocking::get(entry["url"].as_str().unwrap()).unwrap().bytes().unwrap().to_vec();
    println!("{} bytes of {}", bytes.len(), entry["url"]);
    crate::updates::verify(&bytes, entry["signature"].as_str().unwrap()).expect("the real signature");
    let mut bad = bytes.clone();
    bad[100] ^= 1;
    assert!(crate::updates::verify(&bad, entry["signature"].as_str().unwrap()).is_err());
}

#[test]
#[ignore]
fn net_the_resolver_finds_a_copy_and_similar_songs() {
    init();
    let st = crate::state::get();
    let library = crate::library::Library::new(st.data_dir.clone());
    let offline = crate::offline::Offline::new(&st.data_dir);
    let r = crate::player::resolve::Resolver::new(library, offline);
    let found = youtube::search("radiohead creep").expect("search");
    let t = found[0].clone();
    let cancel = Arc::new(AtomicBool::new(false));
    use crate::player::resolve::Resolve;
    let first = r.clone().candidates(&t, cancel).find_map(|c| c.ok()).expect("a source");
    println!("first source: {:?} {:?}", first.quality, first.via.map(|v| v.id));
    let similar = r.similar(&t, &Default::default());
    println!("similar: {}", similar.iter().take(5).map(|t| format!("{} — {}", t.artist_line(), t.title)).collect::<Vec<_>>().join("; "));
    assert!(similar.len() >= 5);
}
