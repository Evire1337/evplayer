import { describe, expect, it } from "vitest";
import { activityFor, decodeFrames, encodeFrame } from "./discord";

describe("discord frames", () => {
  it("round-trips frames and keeps partial data", () => {
    const a = encodeFrame(1, { evt: "READY" });
    const b = encodeFrame(1, { x: 2 });
    const { frames, rest } = decodeFrames(Buffer.concat([a, b.subarray(0, 5)]));
    expect(frames).toEqual([{ op: 1, data: { evt: "READY" } }]);
    expect(rest.length).toBe(5);
  });
  it("builds a listening activity", () => {
    const act = activityFor({ title: "A", artist: "B", startedAt: 1, endsAt: 2, artwork: "https://x/y.jpg" });
    expect(act.type).toBe(2);
    expect(act.details).toBe("A  ");
    expect(act.timestamps).toEqual({ start: 1, end: 2 });
    expect(activityFor({ title: "A", artist: "B", startedAt: 10.6, endsAt: 20.2 }).timestamps).toEqual({ start: 11, end: 20 });
    expect(act.assets.large_image).toBe("https://x/y.jpg");
  });
});

import net from "node:net";
import os from "node:os";
import path from "node:path";
import { mkdtempSync } from "node:fs";
import { DiscordPresence } from "./discord";

describe("discord connection", () => {
  it("handshakes, reads the user and sends the activity", async () => {
    const dir = mkdtempSync(path.join(os.tmpdir(), "dc-"));
    process.env.XDG_RUNTIME_DIR = dir;
    const got: Array<{ op: number; data: any }> = [];
    const server = net.createServer((sock) => {
      sock.on("data", (chunk: Buffer) => {
        const { frames } = decodeFrames(chunk);
        for (const f of frames) {
          got.push(f);
          if (f.op === 0) sock.write(encodeFrame(1, { cmd: "DISPATCH", evt: "READY", data: { user: { id: "42", username: "evire", global_name: "Evire", avatar: "abc" } } }));
        }
      });
    });
    await new Promise<void>((r) => server.listen(path.join(dir, "discord-ipc-0"), r));
    const dc = new DiscordPresence();
    dc.update({ title: "Song", artist: "Artist", startedAt: 1 });
    dc.setAppId("123456789012345678");
    await new Promise((r) => setTimeout(r, 300));
    expect(dc.status().status).toBe("connected");
    expect(dc.status().user).toMatchObject({ username: "evire", globalName: "Evire" });
    expect(got[0]).toMatchObject({ op: 0, data: { client_id: "123456789012345678" } });
    expect(got[1].data.cmd).toBe("SET_ACTIVITY");
    expect(got[1].data.args.activity.details).toBe("Song");
    dc.destroy();
    server.close();
  });
});
