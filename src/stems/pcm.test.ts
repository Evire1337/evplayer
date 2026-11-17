import { describe, expect, it } from "vitest";
import { CHUNKS_AHEAD, CHUNKS_BEHIND, CHUNK_FRAMES, FRAME_BYTES, SOURCE_RATE, WAV_HEADER_BYTES, chunkBytes, chunkOf, framesFromContentRange, pitchCompensation, placeNow, windowAt } from "./pcm";

describe("chunks of a track's file", () => {
  it("puts a frame in its chunk", () => {
    expect(chunkOf(0)).toBe(0);
    expect(chunkOf(CHUNK_FRAMES - 1)).toBe(0);
    expect(chunkOf(CHUNK_FRAMES)).toBe(1);
    expect(chunkOf(-5)).toBe(0);
  });

  it("asks for exactly the bytes of a chunk, after the header", () => {
    expect(chunkBytes(0)).toEqual({ start: WAV_HEADER_BYTES, end: WAV_HEADER_BYTES + CHUNK_FRAMES * FRAME_BYTES - 1 });
    const second = chunkBytes(1);
    expect(second.start).toBe(chunkBytes(0).end + 1);
  });

  it("knows the length of the song from the size the server tells", () => {
    const frames = 5_000_000;
    expect(framesFromContentRange(`bytes 44-${44 + CHUNK_FRAMES * 4 - 1}/${WAV_HEADER_BYTES + frames * FRAME_BYTES}`)).toBe(frames);
    expect(framesFromContentRange(null)).toBeNull();
    expect(framesFromContentRange("bytes */0")).toBeNull();
    expect(framesFromContentRange("bytes 0-1/*")).toBeNull();
  });
});

describe("the window of chunks that is held", () => {
  it("is a little behind and further ahead of the playing place", () => {
    const w = windowAt(CHUNK_FRAMES * 10 + 5, null);
    expect(w).toEqual({ from: 10 - CHUNKS_BEHIND, to: 10 + CHUNKS_AHEAD });
  });

  it("stops at the start and at the end of the song", () => {
    expect(windowAt(0, null).from).toBe(0);
    // 3.5 chunks long: chunks 0 to 3
    expect(windowAt(CHUNK_FRAMES * 3, CHUNK_FRAMES * 3.5).to).toBe(3);
    expect(windowAt(0, 1).to).toBe(0);
  });
});

describe("where the tracks are now", () => {
  it("moves on from the last report at the playing speed", () => {
    expect(placeNow(SOURCE_RATE * 10, 5, 5.5, 1)).toBeCloseTo(10.5);
    expect(placeNow(SOURCE_RATE * 10, 5, 5.5, 2)).toBeCloseTo(11);
    // a report from the future (clocks read at different moments) does not move it back
    expect(placeNow(SOURCE_RATE * 10, 5, 4.9, 1)).toBeCloseTo(10);
  });
});

describe("the pitch the shifter must take back", () => {
  it("is nothing at normal speed and where the pitch moves with the speed", () => {
    expect(pitchCompensation(1, true)).toBe(0);
    expect(pitchCompensation(1.5, false)).toBe(0);
  });

  it("brings the pitch back where the speed should not change it", () => {
    // twice as fast sounds an octave up: an octave down brings it back
    expect(pitchCompensation(2, true)).toBeCloseTo(-12);
    expect(pitchCompensation(0.5, true)).toBeCloseTo(12);
    expect(pitchCompensation(0, true)).toBe(0);
  });
});
