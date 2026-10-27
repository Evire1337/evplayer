import { strToU8, zipSync } from "fflate";
import { describe, expect, it } from "vitest";
import { parseSpotifyExport, readExportFiles } from "./spotifyExport";

const labels = { liked: "Liked", top: "Top", importedFrom: "from Spotify" };

const library = JSON.stringify({
  tracks: [
    { artist: "Daft Punk", album: "Discovery", track: "One More Time", uri: "spotify:track:1" },
    { artist: "Daft Punk", album: "Discovery", track: "One More Time", uri: "spotify:track:1" },
    { artist: "Burial", album: "Untrue", track: "Archangel", uri: "spotify:track:2" },
  ],
});
const playlists = JSON.stringify({
  playlists: [
    { name: "Ночь", items: [{ track: { trackName: "Archangel", artistName: "Burial", albumName: "Untrue", trackUri: "spotify:track:2" } }, { track: null, episode: {} }] },
    { name: "Empty", items: [] },
  ],
});
const history = JSON.stringify([
  { artistName: "A", trackName: "Short", msPlayed: 1000 },
  { artistName: "B", trackName: "Long", msPlayed: 200000 },
  { master_metadata_album_artist_name: "C", master_metadata_track_name: "Ext", ms_played: 500000 },
]);

describe("spotify data export", () => {
  it("turns likes, playlists and history into playlists", () => {
    const out = parseSpotifyExport(
      new Map([
        ["YourLibrary.json", library],
        ["Playlist1.json", playlists],
        ["StreamingHistory_music_0.json", history],
      ]),
      labels,
    );
    expect(out.map((p) => [p.kind, p.title, p.tracks.length])).toEqual([
      ["liked", "Liked", 2],
      ["top", "Top", 2],
      ["playlist", "Ночь", 1],
    ]);
    expect(out[1].tracks[0].title).toBe("Ext");
    expect(out[0].tracks[0]).toMatchObject({ title: "One More Time", artists: ["Daft Punk"], album: "Discovery" });
  });

  it("reads the zip Spotify sends", async () => {
    const zip = zipSync({ "Spotify Account Data/YourLibrary.json": strToU8(library), "Spotify Account Data/ReadMeFirst.pdf": strToU8("x") });
    const file = new File([zip], "my_spotify_data.zip");
    const files = await readExportFiles([file]);
    expect([...files.keys()]).toEqual(["YourLibrary.json"]);
  });
});
