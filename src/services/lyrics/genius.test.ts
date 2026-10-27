import { describe, expect, it } from "vitest";
import { extractGeniusLyrics, pickGeniusHit } from "./genius";

describe("genius", () => {
  it("picks the hit whose title and artist match", () => {
    const hits = [
      { result: { title: "Archangel (Remix)", artist_names: "Someone Else", url: "u1", lyrics_state: "complete" } },
      { result: { title: "Archangel", artist_names: "Burial", url: "u2", lyrics_state: "complete" } },
    ];
    expect(pickGeniusHit(hits, { title: "Archangel", artist: "Burial" })?.url).toBe("u2");
    expect(pickGeniusHit(hits, { title: "Other song", artist: "Burial" })).toBeUndefined();
  });

  it("pulls the lines out of the page and drops section labels and page chrome", () => {
    const html = `<div data-lyrics-container="true"><div data-exclude-from-selection="true">12 Contributors</div>[Куплет 1]<br>Первая строка<br>Вторая строка<br><br>[Припев]<br>Третья строка<i>!</i></div>`;
    expect(extractGeniusLyrics(html)).toBe("Первая строка\nВторая строка\n\nТретья строка!");
  });
});
