import { describe, expect, it } from "vitest";
import { looksLikeTrackList, parseTrackList } from "./trackList";

describe("pasted song lists", () => {
  it("reads 'Artist — Title' lines, numbered or with a length", () => {
    const list = parseTrackList("1. Daft Punk — Get Lucky 4:08\n2) The Weeknd - Blinding Lights\nМакс Корж – Малолетка 3:31");
    expect(list).toEqual([
      { artists: ["Daft Punk"], title: "Get Lucky", durationMs: 248000 },
      { artists: ["The Weeknd"], title: "Blinding Lights" },
      { artists: ["Макс Корж"], title: "Малолетка", durationMs: 211000 },
    ]);
  });

  it("reads rows copied from a VK playlist (title, artist, length)", () => {
    const list = parseTrackList("Эскапист\ndrainedgod\n2:41\nПачка сигарет\nКино\n4:28\n");
    expect(list).toEqual([
      { title: "Эскапист", artists: ["drainedgod"], durationMs: 161000 },
      { title: "Пачка сигарет", artists: ["Кино"], durationMs: 268000 },
    ]);
  });

  it("splits several artists", () => {
    expect(parseTrackList("Kanye West, Lil Pump feat. Adele Givens - I Love It")[0].artists).toEqual(["Kanye West", "Lil Pump", "Adele Givens"]);
  });

  it("is not a link or a code", () => {
    expect(looksLikeTrackList("https://vk.com/music/playlist/1_2")).toBe(false);
    expect(looksLikeTrackList("EVP2.abc")).toBe(false);
    expect(looksLikeTrackList("просто текст")).toBe(false);
    expect(looksLikeTrackList("Кино - Группа крови")).toBe(true);
  });
});
