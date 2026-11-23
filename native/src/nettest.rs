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
