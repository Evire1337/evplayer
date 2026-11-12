import { describe, expect, it } from "vitest";
import { cleanOvhLyrics } from "./lyricsovh";

describe("lyrics.ovh", () => {
  it("drops its header line and extra blank lines", () => {
    expect(cleanOvhLyrics("Paroles de la chanson Song par Artist\r\nFirst line of the song\r\n\r\n\r\n\r\nSecond line of the song")).toBe(
      "First line of the song\n\nSecond line of the song",
    );
  });
  it("says nothing for an empty or tiny answer", () => {
    expect(cleanOvhLyrics(undefined)).toBeUndefined();
    expect(cleanOvhLyrics("  \n ")).toBeUndefined();
    expect(cleanOvhLyrics("la la")).toBeUndefined();
  });
});
