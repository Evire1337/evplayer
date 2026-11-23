//! The queue's rules, as in src/player/store.ts: what is next and previous, shuffling,
//! moving songs around. Plain functions, so they are tested without any sound.

use serde::{Deserialize, Serialize};

use crate::model::Track;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

impl Repeat {
    /// off → the whole queue → one song → off
    pub fn next(self) -> Self {
        match self {
            Repeat::Off => Repeat::All,
            Repeat::All => Repeat::One,
            Repeat::One => Repeat::Off,
        }
    }
}

/// the next song's index, or None at the end of the queue
pub fn next_index(len: usize, index: usize, repeat: Repeat, manual: bool) -> Option<usize> {
    if len == 0 {
        return None;
    }
    if repeat == Repeat::One && !manual {
        return Some(index);
    }
    if index + 1 < len {
        return Some(index + 1);
    }
    // at the end: start over only with "repeat all"; otherwise the player decides
    // (similar songs when autoplay is on)
    (repeat == Repeat::All).then_some(0)
}

pub fn prev_index(len: usize, index: usize, repeat: Repeat) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some(if index > 0 {
        index - 1
    } else if repeat == Repeat::All {
        len - 1
    } else {
        0
    })
}

/// Fisher-Yates, keeping the given song first
pub fn shuffled(queue: &[Track], first: Option<&str>) -> Vec<Track> {
    let mut rest: Vec<Track> = queue.iter().filter(|t| Some(t.id.as_str()) != first).cloned().collect();
    for i in (1..rest.len()).rev() {
        let j = rand::random::<usize>() % (i + 1);
        rest.swap(i, j);
    }
    match first.and_then(|id| queue.iter().find(|t| t.id == id)) {
        Some(f) => std::iter::once(f.clone()).chain(rest).collect(),
        None => rest,
    }
}

/// the same songs, in any order (a shuffled queue is still the same list)
pub fn same_songs(a: &[Track], b: &[Track]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let ids: std::collections::HashSet<&str> = a.iter().map(|t| t.id.as_str()).collect();
    b.iter().all(|t| ids.contains(t.id.as_str()))
}

/// move a song in the queue; the playing song keeps playing wherever it ends up
pub fn move_in_queue(queue: &mut Vec<Track>, index: usize, from: usize, to: usize) -> usize {
    if from == to || from >= queue.len() {
        return index;
    }
    let item = queue.remove(from);
    let target = to.min(queue.len());
    queue.insert(target, item);
    let mut index = index as isize;
    if from == index as usize {
        return target;
    }
    if from < index as usize {
        index -= 1;
    }
    if target as isize <= index {
        index += 1;
    }
    index as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(id: &str) -> Track {
        Track { id: id.into(), ..Default::default() }
    }

    #[test]
    fn next_and_previous() {
        assert_eq!(next_index(3, 0, Repeat::Off, false), Some(1));
        assert_eq!(next_index(3, 2, Repeat::Off, false), None);
        assert_eq!(next_index(3, 2, Repeat::All, false), Some(0));
        assert_eq!(next_index(3, 1, Repeat::One, false), Some(1));
        assert_eq!(next_index(3, 1, Repeat::One, true), Some(2));
        assert_eq!(next_index(0, 0, Repeat::All, true), None);
        assert_eq!(prev_index(3, 0, Repeat::Off), Some(0));
        assert_eq!(prev_index(3, 0, Repeat::All), Some(2));
        assert_eq!(prev_index(3, 2, Repeat::Off), Some(1));
        assert_eq!(Repeat::Off.next().next().next(), Repeat::Off);
    }

    #[test]
    fn shuffle_keeps_the_song_first_and_all_songs() {
        let q: Vec<Track> = (0..20).map(|i| t(&i.to_string())).collect();
        let s = shuffled(&q, Some("7"));
        assert_eq!(s[0].id, "7");
        assert!(same_songs(&q, &s));
    }

    #[test]
    fn moving_keeps_the_playing_song() {
        let mut q: Vec<Track> = ["a", "b", "c", "d"].iter().map(|s| t(s)).collect();
        // c (playing, index 2) stays playing when a goes after it
        let i = move_in_queue(&mut q, 2, 0, 3);
        assert_eq!(q[i].id, "c");
        let i = move_in_queue(&mut q, i, i, 0);
        assert_eq!((i, q[0].id.as_str()), (0, "c"));
    }
}
