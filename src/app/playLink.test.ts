import { describe, expect, it, vi } from "vitest";

vi.mock("@/player/engine", () => ({ player: { playQueue: vi.fn() } }));
vi.mock("@/services/soundcloud/api", () => ({ scGet: vi.fn() }));

import type { Track } from "@/types";
import { LISTEN_PAGE, listenUrl, trackFromLink } from "./playLink";

const song: Track = {
  id: "sc:12345",
  source: "soundcloud",
  title: "VVV (HE'S BACK)",
  artists: [{ id: "scu:1", name: "Mikeey" }],
  durationMs: 154_000,
  artwork: "https://i1.sndcdn.com/artworks-x-t500x500.jpg",
  permalink: "https://soundcloud.com/mikeeysmind-scmusic/vvv-hes-back",
};

/** what the page does: the same query, handed to the app */
const appLink = (page: string) => `evplayer://play?${new URL(page).search.slice(1)}`;

describe("song links", () => {
  it("goes through the page into the app and back to the same song", () => {
    const page = listenUrl(song)!;
    expect(page.startsWith(LISTEN_PAGE)).toBe(true);
    expect(trackFromLink(appLink(page))).toMatchObject({ id: "sc:12345", source: "soundcloud", title: "VVV (HE'S BACK)", durationMs: 154_000, permalink: song.permalink, artwork: song.artwork });
  });

  it("knows YouTube songs and nothing else", () => {
    const yt = listenUrl({ ...song, id: "yt:dQw4w9WgXcQ", source: "youtube" })!;
    expect(trackFromLink(appLink(yt))?.source).toBe("youtube");
    expect(listenUrl({ ...song, id: "local:abc", source: "local" })).toBeUndefined();
    expect(trackFromLink("evplayer://play?id=local:abc")).toBeNull();
    expect(trackFromLink("evplayer://jam/JAM-7K9P-2QXZ")).toBeNull();
    expect(trackFromLink("evplayer://play?id=sc:1&art=javascript:alert(1)")?.artwork).toBeUndefined();
  });

  it("stays within Discord's 512 characters", () => {
    expect(listenUrl({ ...song, title: "x".repeat(400), artwork: `https://i1.sndcdn.com/${"a".repeat(300)}.jpg` })!.length).toBeLessThanOrEqual(512);
  });
});