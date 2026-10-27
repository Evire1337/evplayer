import { strFromU8, unzipSync } from "fflate";
import type { NamedTrack } from "@/library/matchTracks";

type ClientImportedTrack = NamedTrack & { sourceId: string; album?: string };
interface ClientImportedPlaylist {
  title: string;
  description?: string;
  tracks: ClientImportedTrack[];
}

// Spotify's "Download your data" export (Account privacy -> Download your data). Any account
// can request it, Premium or not. Relevant files:
//   YourLibrary.json                      liked songs ("tracks": [{artist, album, track, uri}])
//   Playlist1.json / Playlists1.json ...   playlists ("playlists": [{name, items: [{track: {...}}]}])
//   StreamingHistory_music_0.json         short history ([{artistName, trackName, msPlayed}])
//   Streaming_History_Audio_*.json        extended history ([{master_metadata_*, ms_played}])
// The files hold names only, no audio; songs are matched to SoundCloud / YouTube on import.

export interface SpotifyExportPlaylist extends ClientImportedPlaylist {
  kind: "liked" | "playlist" | "top";
}

export interface SpotifyExportLabels {
  liked: string;
  top: string;
  importedFrom: string;
}

const TOP_LIMIT = 100;

function str(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

function track(title: string, artist: string, album: string, uri: string): ClientImportedTrack | null {
  if (!title) return null;
  return {
    sourceId: uri || `spotify:${artist}|${title}`.toLowerCase(),
    title,
    artists: artist ? [artist] : [],
    ...(album ? { album } : {}),
  };
}

function dedupe(tracks: ClientImportedTrack[]): ClientImportedTrack[] {
  const seen = new Set<string>();
  return tracks.filter((t) => {
    const key = `${t.title}|${t.artists[0] ?? ""}`.toLowerCase();
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

/** read the chosen files (json, or the whole zip as Spotify sends it) into name -> text */
export async function readExportFiles(files: File[]): Promise<Map<string, string>> {
  const out = new Map<string, string>();
  for (const file of files) {
    const name = file.name;
    if (/\.zip$/i.test(name)) {
      const entries = unzipSync(new Uint8Array(await file.arrayBuffer()), {
        filter: (entry) => /\.json$/i.test(entry.name),
      });
      for (const [entryName, data] of Object.entries(entries)) {
        out.set(entryName.split("/").pop() ?? entryName, strFromU8(data));
      }
    } else if (/\.json$/i.test(name)) {
      out.set(name, await file.text());
    }
  }
  return out;
}

export function parseSpotifyExport(files: Map<string, string>, labels: SpotifyExportLabels): SpotifyExportPlaylist[] {
  const result: SpotifyExportPlaylist[] = [];
  const history = new Map<string, { t: ClientImportedTrack; ms: number }>();
  const base = { description: labels.importedFrom };

  for (const [name, text] of files) {
    let json: unknown;
    try {
      json = JSON.parse(text);
    } catch {
      continue;
    }
    const record = json && typeof json === "object" ? (json as Record<string, unknown>) : null;

    // liked songs
    if (record && Array.isArray(record.tracks) && /library/i.test(name)) {
      const tracks = dedupe(
        record.tracks.flatMap((row) => {
          const r = row as Record<string, unknown>;
          const t = track(str(r.track), str(r.artist), str(r.album), str(r.uri));
          return t ? [t] : [];
        }),
      );
      if (tracks.length) result.push({ ...base, kind: "liked", title: labels.liked, tracks });
      continue;
    }

    // playlists
    if (record && Array.isArray(record.playlists)) {
      for (const raw of record.playlists) {
        const p = raw as Record<string, unknown>;
        const items = Array.isArray(p.items) ? p.items : [];
        const tracks = dedupe(
          items.flatMap((row) => {
            const t = (row as Record<string, unknown>)?.track as Record<string, unknown> | null;
            if (!t) return [];
            const imported = track(str(t.trackName), str(t.artistName), str(t.albumName), str(t.trackUri));
            return imported ? [imported] : [];
          }),
        );
        const title = str(p.name) || "Spotify";
        if (tracks.length) {
          result.push({ ...base, kind: "playlist", title, ...(str(p.description) ? { description: str(p.description) } : {}), tracks });
        }
      }
      continue;
    }

    // listening history (short or extended format)
    if (Array.isArray(json) && /history/i.test(name)) {
      for (const raw of json) {
        const r = raw as Record<string, unknown>;
        const title = str(r.trackName) || str(r.master_metadata_track_name);
        const artist = str(r.artistName) || str(r.master_metadata_album_artist_name);
        const album = str(r.master_metadata_album_album_name);
        const ms = Number(r.msPlayed ?? r.ms_played) || 0;
        const t = track(title, artist, album, str(r.spotify_track_uri));
        if (!t || ms < 30_000) continue;
        const key = `${title}|${artist}`.toLowerCase();
        const row = history.get(key) ?? { t, ms: 0 };
        row.ms += ms;
        history.set(key, row);
      }
    }
  }

  if (history.size) {
    const tracks = [...history.values()].sort((a, b) => b.ms - a.ms).slice(0, TOP_LIMIT).map((row) => row.t);
    result.push({ ...base, kind: "top", title: labels.top, tracks });
  }

  const order = { liked: 0, top: 1, playlist: 2 };
  return result.sort((a, b) => order[a.kind] - order[b.kind]);
}
