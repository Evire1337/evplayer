import { describe, expect, it, vi } from "vitest";
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
import { mkdtempSync, rmSync } from "node:fs";
import { DiscordPresence } from "./discord";

describe("discord connection", () => {
  it("handshakes, reads the user and sends the activity", async () => {
    // Windows has no unix sockets: Discord listens on a named pipe there, and a
    // filesystem path cannot be listened on at all (EACCES, which broke CI).
    const dir = mkdtempSync(path.join(os.tmpdir(), "dc-"));
    const address =
      process.platform === "win32"
        ? `\\\\.\\pipe\\evplayer-discord-test-${process.pid}-${Date.now()}`
        : path.join(dir, "discord-ipc-0");

    const got: Array<{ op: number; data: any }> = [];
    const peers: net.Socket[] = [];
    const server = net.createServer((sock) => {
      peers.push(sock);
      sock.on("data", (chunk: Buffer) => {
        const { frames } = decodeFrames(chunk);
        for (const f of frames) {
          got.push(f);
          if (f.op === 0) sock.write(encodeFrame(1, { cmd: "DISPATCH", evt: "READY", data: { user: { id: "42", username: "evire", global_name: "Evire", avatar: "abc" } } }));
        }
      });
    });
    await new Promise<void>((resolve, reject) => {
      server.once("error", reject);
      server.listen(address, resolve);
    });

    const dc = new DiscordPresence(() => address);
    try {
      dc.update({ title: "Song", artist: "Artist", startedAt: 1 });
      dc.setAppId("123456789012345678");
      await vi.waitFor(
        () => {
          expect(dc.status().status).toBe("connected");
          expect(got.length).toBeGreaterThanOrEqual(2);
        },
        { timeout: 3000 },
      );
      expect(dc.status().user).toMatchObject({ username: "evire", globalName: "Evire" });
      expect(got[0]).toMatchObject({ op: 0, data: { client_id: "123456789012345678" } });
      expect(got[1].data.cmd).toBe("SET_ACTIVITY");
      expect(got[1].data.args.activity.details).toBe("Song");
    } finally {
      dc.destroy();
      peers.forEach((s) => s.destroy());
      await new Promise<void>((resolve) => server.close(() => resolve()));
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
