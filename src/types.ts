export type TrackSource = "soundcloud" | "youtube" | "local";

export interface Artist {
  id: string;
  name: string;
}

export interface Track {
  /** "sc:123" for SoundCloud, "yt:VIDEO_ID" for YouTube, "local:..." for your files */
  id: string;
  source: TrackSource;
  title: string;
  artists: Artist[];
  durationMs: number;
  artwork?: string;
  /** plays/views on the service */
  plays?: number;
  permalink?: string;
}

export interface ArtistSummary {
  id: string;
  name: string;
  avatar?: string;
  followers?: number;
}

/** an album or a playlist on a service */
export interface CollectionSummary {
  /** "scp:123" for SoundCloud, "ytb:MPREb…" / "ytb:VLPL…" for YouTube Music */
  id: string;
  kind: "album" | "playlist";
  title: string;
  subtitle: string;
  artwork?: string;
  year?: number;
  trackCount?: number;
  artist?: { id: string; name: string };
}
