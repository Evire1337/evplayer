import { describe, expect, it } from "vitest";
import { artworkFallbacks } from "./Artwork";
import { artworkUrl } from "@/services/soundcloud/mappers";

describe("artwork", () => {
  it("skips SoundCloud's placeholder and offers smaller sizes", () => {
    expect(artworkUrl("https://a1.sndcdn.com/images/default_avatar_large.png")).toBeUndefined();
    expect(artworkFallbacks("https://i1.sndcdn.com/artworks-abc-t500x500.jpg")).toEqual([
      "https://i1.sndcdn.com/artworks-abc-t300x300.jpg",
      "https://i1.sndcdn.com/artworks-abc-large.jpg",
      "https://i1.sndcdn.com/artworks-abc-original.jpg",
    ]);
    expect(artworkFallbacks("https://i.ytimg.com/vi/XYZ/maxresdefault.jpg")[0]).toBe("https://i.ytimg.com/vi/XYZ/hqdefault.jpg");
  });
});
