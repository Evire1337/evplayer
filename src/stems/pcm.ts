// How a split song's tracks reach the player (public/worklets/stem-player.js): each track is a
// 16-bit stereo 44.1 kHz WAV file, read in chunks of a fixed length and only around the place
// that plays, so a song of any length takes the same small amount of memory. These are the pure
// rules; the fetching and the node are in player.ts.

/** the rate the tracks are stored at (stems.rs) */
export const SOURCE_RATE = 44100;
/** frames per chunk; must match the worklet (a power of two: about 3 seconds) */
export const CHUNK_FRAMES = 1 << 17;
/** the size of the WAV header in front of the sound (stems.rs writes exactly this much) */
export const WAV_HEADER_BYTES = 44;
/** bytes per frame: 16-bit stereo */
export const FRAME_BYTES = 4;

/** chunks kept behind the playing place (a short seek back costs nothing) */
export const CHUNKS_BEHIND = 1;
/** chunks kept ahead of it: about 15 seconds, far more than the disk needs to be in time */
export const CHUNKS_AHEAD = 5;
/** chunks that must be in the player before it starts: the one that plays and the next */
export const CHUNKS_TO_START = 2;

/** the chunk a frame is in */
export const chunkOf = (frame: number): number => Math.max(0, Math.floor(frame / CHUNK_FRAMES));

/** the bytes of a chunk in a track's file, as the inclusive range of an HTTP Range request */
export function chunkBytes(index: number): { start: number; end: number } {
  const start = WAV_HEADER_BYTES + index * CHUNK_FRAMES * FRAME_BYTES;
  return { start, end: start + CHUNK_FRAMES * FRAME_BYTES - 1 };
}

/** the length of a song in frames, from the file size an HTTP `Content-Range: bytes a-b/size` tells; null if it does not */
export function framesFromContentRange(header: string | null): number | null {
  const size = Number(header?.match(/\/(\d+)\s*$/)?.[1]);
  return Number.isFinite(size) && size >= WAV_HEADER_BYTES ? Math.floor((size - WAV_HEADER_BYTES) / FRAME_BYTES) : null;
}

/** the chunks the player should hold when it plays at `frame` (`total` frames long, null if not known yet) */
export function windowAt(frame: number, total: number | null): { from: number; to: number } {
  const at = chunkOf(frame);
  const last = total === null ? Infinity : Math.max(0, Math.ceil(total / CHUNK_FRAMES) - 1);
  return { from: Math.max(0, at - CHUNKS_BEHIND), to: Math.min(last, at + CHUNKS_AHEAD) };
}

// ---- keeping the tracks with the song ------------------------------------------------------

/** a gap below this is not heard between the tracks and the deck that keeps the song's time */
export const SYNC_OK_S = 0.12;
/**
 * Where the tracks are now, from the last thing the player said: its place, how long ago (in
 * context seconds) it said it, and how fast it plays.
 */
export function placeNow(frame: number, reportedAt: number, now: number, rate: number): number {
  return frame / SOURCE_RATE + Math.max(0, now - reportedAt) * rate;
}

/** the tracks' place against the deck's, in seconds (positive: the tracks are ahead) */
export const gapS = (tracksAt: number, deckAt: number): number => tracksAt - deckAt;

/**
 * The speed change as the graph needs it: the tracks play faster or slower like a record
 * (so the pitch moves with the speed), and where the deck keeps the pitch while the speed changes,
 * the pitch shifter takes that back. Returns how many semitones the shifter must add.
 */
export function pitchCompensation(rate: number, preservesPitch: boolean): number {
  if (!preservesPitch || !Number.isFinite(rate) || rate <= 0 || Math.abs(rate - 1) < 1e-6) return 0;
  return -12 * Math.log2(rate);
}
