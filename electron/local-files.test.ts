// @vitest-environment node
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { beforeAll, describe, expect, it } from "vitest";
import { LocalLibrary, localTrackId } from "./local-files";

let dir = "";
let hasFfmpeg = true;

beforeAll(() => {
  dir = mkdtempSync(path.join(os.tmpdir(), "evp-local-"));
  try {
    execFileSync("ffmpeg", ["-loglevel", "error", "-y", "-f", "lavfi", "-i", "sine=frequency=440:duration=2", "-metadata", "title=Тест", "-metadata", "artist=Артист", path.join(dir, "a.flac")]);
    execFileSync("ffmpeg", ["-loglevel", "error", "-y", "-f", "lavfi", "-i", "sine=frequency=440:duration=1", path.join(dir, "01 - Some Band - Song Name.mp3")]);
  } catch {
    hasFfmpeg = false;
  }
  writeFileSync(path.join(dir, "notes.txt"), "not audio");
});

describe("LocalLibrary", () => {
  it("adds a folder, reads tags, falls back to the file name and skips non-audio", async () => {
    if (!hasFfmpeg) return;
    const lib = new LocalLibrary(path.join(dir, "data"));
    const { added } = await lib.add([dir]);
    const byTitle = Object.fromEntries(added.map((t) => [t.title, t]));
    expect(byTitle["Тест"]).toMatchObject({ artists: ["Артист"], lossless: true });
    expect(byTitle["Тест"].durationMs).toBeGreaterThan(1500);
    expect(byTitle["Song Name"].artists).toEqual(["Some Band"]);
    expect(added).toHaveLength(2);

    const again = new LocalLibrary(path.join(dir, "data"));
    expect(await again.list()).toHaveLength(2);
  });

  it("serves byte ranges with CORS so seeking works", async () => {
    if (!hasFfmpeg) return;
    const lib = new LocalLibrary(path.join(dir, "data2"));
    await lib.add([path.join(dir, "a.flac")]);
    const id = localTrackId(path.join(dir, "a.flac"));
    const res = await lib.handle(new Request(`evplayer-media://track/${encodeURIComponent(id)}`, { headers: { range: "bytes=0-99" } }));
    expect(res.status).toBe(206);
    expect(res.headers.get("access-control-allow-origin")).toBe("*");
    expect((await res.arrayBuffer()).byteLength).toBe(100);
    const missing = await lib.handle(new Request("evplayer-media://track/local%3Anope"));
    expect(missing.status).toBe(404);
  });

  it("keeps edited tags over a rescan and follows files added to or removed from a folder", async () => {
    if (!hasFfmpeg) return;
    const music = path.join(dir, "watched");
    mkdirSync(music, { recursive: true });
    copyFileSync(path.join(dir, "a.flac"), path.join(music, "one.flac"));
    const lib = new LocalLibrary(path.join(dir, "data3"));
    await lib.add([music]);
    expect(await lib.watchedFolders()).toEqual([path.resolve(music)]);
    const id = localTrackId(path.join(music, "one.flac"));
    await lib.update(id, { title: "Моё название", artists: ["Я"], cover: "data:image/png;base64,iVBORw0KGgo=" });

    copyFileSync(path.join(dir, "a.flac"), path.join(music, "two.flac"));
    expect(await lib.rescan(music)).toBe(true);
    await lib.add([music]);
    const records = await lib.list();
    expect(records).toHaveLength(2);
    expect(records.find((r) => r.id === id)).toMatchObject({ title: "Моё название", artists: ["Я"], hasCover: true, edited: true });

    rmSync(path.join(music, "two.flac"));
    expect(await lib.rescan(music)).toBe(true);
    expect(await lib.list()).toHaveLength(1);
    lib.stopWatching();
  });
});
