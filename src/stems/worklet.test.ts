import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

// The player's worklet runs here against a stand-in for the audio thread: the test pushes
// chunks and messages in and pulls 128-frame blocks out, as the browser does.

const CHUNK = 1 << 17;
const SOURCE = readFileSync(resolve(process.cwd(), "public/worklets/stem-player.js"), "utf8");

interface Posted {
  type: string;
  frame?: number;
  playing?: boolean;
  starved?: boolean;
}

interface Processor {
  port: { onmessage: ((e: { data: unknown }) => void) | null; postMessage: (m: Posted) => void };
  process: (inputs: Float32Array[][], outputs: Float32Array[][]) => boolean;
}

function make(sampleRate = 44100) {
  let Class: (new () => Processor) | null = null;
  const posted: Posted[] = [];
  class Base {
    port = { onmessage: null as ((e: { data: unknown }) => void) | null, postMessage: (m: Posted) => void posted.push(m) };
  }
  const state = { currentTime: 0 };
  new Function("AudioWorkletProcessor", "registerProcessor", "sampleRate", "currentTime", SOURCE)(
    Base,
    (_name: string, c: new () => Processor) => (Class = c),
    sampleRate,
    0,
  );
  const node = new (Class as unknown as new () => Processor)();
  const send = (data: unknown) => node.port.onmessage?.({ data });
  const block = () => {
    const out = Array.from({ length: 8 }, () => new Float32Array(128));
    node.process([], [out]);
    state.currentTime += 128 / sampleRate;
    return out;
  };
  /** the first channel (vocals, left) of `blocks` blocks, in one array */
  const run = (blocks: number, channel = 0): Float32Array => {
    const all = new Float32Array(blocks * 128);
    for (let b = 0; b < blocks; b++) all.set(block()[channel], b * 128);
    return all;
  };
  return { node, send, run, block, posted };
}

/** a chunk: each of the four tracks holds a different sine (so a mix-up of tracks would be heard) */
function chunk(index: number, frames = CHUNK, freq = [440, 880, 220, 1760]) {
  return freq.map((f) => {
    const data = new Int16Array(frames * 2);
    for (let i = 0; i < frames; i++) {
      const t = (index * CHUNK + i) / 44100;
      data[i * 2] = Math.round(Math.sin(2 * Math.PI * f * t) * 20000);
      data[i * 2 + 1] = Math.round(Math.sin(2 * Math.PI * f * t) * 10000);
    }
    return data;
  });
}

const give = (send: (m: unknown) => void, index: number, frames = CHUNK) => {
  const parts = chunk(index, frames);
  send({ type: "chunk", index, parts: parts.map((p) => p.buffer) });
  return parts;
};

describe("the stem player worklet", () => {
  it("plays the samples exactly at 44.1 kHz and speed 1, across chunk borders, each track on its own channels", () => {
    const { send, block } = make();
    const c0 = give(send, 0);
    const c1 = give(send, 1);
    const start = CHUNK - 1024;
    send({ type: "seek", frame: start });
    send({ type: "play", on: true });
    const outs: Float32Array[][] = [];
    for (let b = 0; b < 16; b++) outs.push(block());
    // after the 12 ms fade in, frame f of the song is output frame f - start
    for (const f of [CHUNK - 100, CHUNK - 1, CHUNK, CHUNK + 5, CHUNK + 100]) {
      const o = f - start;
      const parts = f < CHUNK ? c0 : c1;
      const off = f < CHUNK ? f : f - CHUNK;
      for (let p = 0; p < 4; p++) {
        for (let ch = 0; ch < 2; ch++) expect(outs[Math.floor(o / 128)][p * 2 + ch][o % 128]).toBeCloseTo(parts[p][off * 2 + ch] / 32768, 6);
      }
    }
  });

  it("fades in rather than starting with a click, and fades out to silence on a stop", () => {
    const { send, block } = make();
    give(send, 0);
    send({ type: "play", on: true });
    const first = block();
    expect(Math.abs(first[0][0])).toBeLessThan(0.01);
    for (let b = 0; b < 8; b++) block();
    send({ type: "play", on: false });
    for (let b = 0; b < 8; b++) block();
    const silent = block();
    expect(Math.max(...silent.map((c) => Math.max(...c.map(Math.abs))))).toBe(0);
  });

  it("does not move on while it is stopped, and goes on from where it stopped", () => {
    const { send, run, posted } = make();
    give(send, 0);
    send({ type: "play", on: true });
    run(40);
    send({ type: "play", on: false });
    run(10);
    // asking again after the fade out is over: the place is the same however long it has stood
    send({ type: "play", on: false });
    const stopped = posted[posted.length - 1].frame as number;
    run(200);
    send({ type: "play", on: false });
    expect(posted[posted.length - 1].frame).toBe(stopped);
    send({ type: "play", on: true });
    expect(posted[posted.length - 1].frame).toBe(stopped);
    run(4);
    expect(run(1).some((v) => v !== 0)).toBe(true);
  });

  it("plays a seek from the new place", () => {
    const { send, run, block } = make();
    give(send, 0);
    send({ type: "seek", frame: 50_000 });
    send({ type: "play", on: true });
    run(8);
    const out = block();
    // 50_000 + 8 * 128 = frame of the first sample of this block
    const frame = 50_000 + 8 * 128;
    const data = chunk(0)[0];
    expect(out[0][0]).toBeCloseTo(data[frame * 2] / 32768, 6);
  });

  it("fades out what plays before it jumps to the place of a seek, with no click", () => {
    const { send, run, posted } = make();
    give(send, 0);
    send({ type: "play", on: true });
    run(20);
    send({ type: "seek", frame: 90_000 });
    const out = run(12);
    let worst = 0;
    for (let i = 1; i < out.length; i++) worst = Math.max(worst, Math.abs(out[i] - out[i - 1]));
    // a 440 Hz sine of 0.61 never steps more than this between two frames; a cut from its peak to zero would
    expect(worst).toBeLessThan(0.06);
    // it went there, and plays on from it
    run(20);
    const frame = posted.filter((m) => m.type === "pos").at(-1)?.frame as number;
    expect(frame).toBeGreaterThan(90_000);
    expect(frame).toBeLessThan(90_000 + 20 * 128 + 12 * 128);
  });

  it("does not cut a fade out short when the seek comes while it is still fading", () => {
    const { send, run, block } = make();
    give(send, 0);
    send({ type: "play", on: true });
    run(20);
    // the deck started to seek: the tracks fade out; its seek follows a few blocks later
    send({ type: "play", on: false });
    const fading = run(2);
    send({ type: "seek", frame: 90_000 });
    send({ type: "play", on: true });
    const after = run(16);
    const all = new Float32Array(fading.length + after.length);
    all.set(fading);
    all.set(after, fading.length);
    let worst = 0;
    for (let i = 1; i < all.length; i++) worst = Math.max(worst, Math.abs(all[i] - all[i - 1]));
    expect(worst).toBeLessThan(0.06);
    expect(block()[0].some((v) => v !== 0)).toBe(true);
  });

  it("keeps the chunks of the place it is fading out of until the fade is over, even if the window moved on", () => {
    // the cut would be loudest at a crest of the wave: try several places in it
    for (const blocks of [20, 21, 22, 23, 24, 25, 26, 27]) {
      const { send, run, block } = make();
      give(send, 0);
      send({ type: "play", on: true });
      run(blocks);
      const before = run(1);
      // the deck began to seek: fade out, and at once the window moves to the new place
      send({ type: "play", on: false });
      send({ type: "keep", from: 5, to: 9 });
      give(send, 6);
      const after = run(8);
      const out = new Float32Array(before.length + after.length);
      out.set(before);
      out.set(after, before.length);
      let worst = 0;
      for (let i = 1; i < out.length; i++) worst = Math.max(worst, Math.abs(out[i] - out[i - 1]));
      expect(worst).toBeLessThan(0.06);
      // then the old chunk is let go, and the new place plays
      send({ type: "seek", frame: 6 * CHUNK + 100 });
      send({ type: "play", on: true });
      run(8);
      expect(block()[0].some((v) => v !== 0)).toBe(true);
    }
  });

  it("lets go of chunks behind the place while it plays on, so memory does not grow", () => {
    const { send, run, posted } = make();
    for (let i = 0; i < 4; i++) give(send, i);
    send({ type: "seek", frame: 2 * CHUNK + 10 });
    send({ type: "play", on: true });
    run(20);
    send({ type: "keep", from: 2, to: 3 });
    run(4);
    // chunk 0 is gone at once: a seek back to it has nothing to play until it is given again
    send({ type: "seek", frame: 100 });
    run(30);
    expect(posted.filter((m) => m.type === "pos").at(-1)?.starved).toBe(true);
    give(send, 0);
    run(30);
    expect(posted.filter((m) => m.type === "pos").at(-1)?.starved).toBe(false);
  });

  it("outputs silence and waits when a chunk has not arrived, then goes on from the same place", () => {
    const { send, run, posted } = make();
    give(send, 0);
    send({ type: "seek", frame: CHUNK - 600 });
    send({ type: "play", on: true });
    run(3);
    // the chunk after this one is missing: it plays up to the border, then waits
    const wait = run(40);
    expect(wait.slice(-128).every((v) => v === 0)).toBe(true);
    const where = posted.filter((m) => m.type === "pos").at(-1);
    expect(where?.starved).toBe(true);
    const stuck = where?.frame as number;
    expect(stuck).toBeGreaterThan(CHUNK - 600);
    expect(stuck).toBeLessThan(CHUNK + 3);
    run(20);
    expect(posted.filter((m) => m.type === "pos").at(-1)?.frame).toBe(stuck);
    give(send, 1);
    run(40);
    const after = posted.filter((m) => m.type === "pos").at(-1);
    expect(after?.starved).toBe(false);
    expect(after?.frame as number).toBeGreaterThan(stuck + 1000);
  });

  it("steps through the song at 44.1 kHz over the context's rate and the playing speed", () => {
    const rates: Array<[number, number]> = [
      [48000, 1],
      [44100, 0.5],
      [44100, 1.5],
      [48000, 2],
    ];
    for (const [sampleRate, speed] of rates) {
      const { send, run, posted } = make(sampleRate);
      give(send, 0);
      give(send, 1);
      send({ type: "rate", value: speed });
      send({ type: "play", on: true });
      run(100);
      const frame = posted.filter((m) => m.type === "pos").at(-1)?.frame as number;
      // the last report is not the last block: allow what one report interval can add
      expect(frame).toBeLessThanOrEqual(100 * 128 * (44100 / sampleRate) * speed + 1);
      expect(frame).toBeGreaterThan(100 * 128 * (44100 / sampleRate) * speed - 2048 * (44100 / sampleRate) * speed - 128);
    }
  });

  it("reads a smooth curve between samples when it resamples (no steps, no overshoot)", () => {
    const { send, run } = make(48000);
    give(send, 0);
    send({ type: "play", on: true });
    const out = run(60);
    let worst = 0;
    for (let i = 2000; i < 7000; i++) worst = Math.max(worst, Math.abs(out[i] - out[i - 1]));
    // 440 Hz at 20000/32768 amplitude: the biggest step between two 48 kHz frames is 2π·440/48000·0.61
    expect(worst).toBeLessThan(((2 * Math.PI * 440) / 48000) * 0.62 + 0.002);
    const peak = Math.max(...out.slice(2000, 7000).map(Math.abs));
    expect(peak).toBeLessThanOrEqual(20000 / 32768 + 0.002);
  });

  it("says when the song is over and stops", () => {
    const { send, run, posted } = make();
    give(send, 0, 4000);
    send({ type: "length", frames: 4000 });
    send({ type: "play", on: true });
    run(60);
    expect(posted.some((m) => m.type === "end")).toBe(true);
    const out = run(2);
    expect(out.every((v) => v === 0)).toBe(true);
  });

  it("lets chunks go outside the window it is told to keep", () => {
    const { send, run, posted } = make();
    give(send, 0);
    give(send, 1);
    send({ type: "keep", from: 1, to: 1 });
    send({ type: "play", on: true });
    run(4);
    // chunk 0 is gone: it cannot play
    expect(posted.filter((m) => m.type === "pos").at(-1)?.starved).toBe(true);
    send({ type: "seek", frame: CHUNK + 10 });
    run(8);
    expect(posted.filter((m) => m.type === "pos").at(-1)?.starved).toBe(false);
  });
});
