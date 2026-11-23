//! A song, an artist, an album or playlist on a service: the same shapes as src/types.ts, so
//! what one build saved the other reads as it is.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    #[default]
    Soundcloud,
    Youtube,
    Local,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Artist {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    /// "sc:123" for SoundCloud, "yt:VIDEO_ID" for YouTube, "local:..." for your files
    pub id: String,
    pub source: Source,
    pub title: String,
    #[serde(default)]
    pub artists: Vec<Artist>,
    #[serde(default)]
    pub duration_ms: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artwork: Option<String>,
    /// plays/views on the service
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plays: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permalink: Option<String>,
    /// the service gives out only a short preview of it (SoundCloud Go+): it plays from elsewhere
    #[serde(default, skip_serializing_if = "is_false")]
    pub preview: bool,
    /// YouTube Music marks it "explicit": the uncensored version
    #[serde(default, skip_serializing_if = "is_false")]
    pub explicit: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl Track {
    /// "Artist, Other Artist"
    pub fn artist_line(&self) -> String {
        let names: Vec<&str> = self.artists.iter().map(|a| a.name.as_str()).filter(|n| !n.is_empty()).collect();
        names.join(", ")
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct ArtistSummary {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub followers: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CollectionKind {
    Album,
    #[default]
    Playlist,
}

/// an album or a playlist on a service
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CollectionSummary {
    /// "scp:123" for SoundCloud, "ytb:MPREb…" / "ytb:VLPL…" for YouTube Music
    pub id: String,
    pub kind: CollectionKind,
    pub title: String,
    #[serde(default)]
    pub subtitle: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artwork: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artist: Option<Artist>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_writes_the_page_s_track() {
        let json = r#"{"id":"sc:1","source":"soundcloud","title":"A","artists":[{"id":"scu:2","name":"B"}],"durationMs":1000,"artwork":"https://i1.sndcdn.com/x-t500x500.jpg","plays":5,"preview":true}"#;
        let t: Track = serde_json::from_str(json).unwrap();
        assert_eq!(t.source, Source::Soundcloud);
        assert!(t.preview && !t.explicit);
        let back = serde_json::to_string(&t).unwrap();
        assert!(back.contains("\"durationMs\":1000.0") || back.contains("\"durationMs\":1000"));
        assert!(!back.contains("explicit"), "{back}");
        let again: Track = serde_json::from_str(&back).unwrap();
        assert_eq!(again, t);
    }
}
