import net from "node:net";
import path from "node:path";
import os from "node:os";

// "Listening to …" in the Discord profile. Talks to the Discord desktop app over its local
// RPC pipe. Discord needs an application id: the user makes one (free) at
// discord.com/developers/applications, the name of that application is what Discord shows.

export interface PresenceState {
  title: string;
  artist: string;
  artwork?: string;
  url?: string;
  /** epoch ms when the song started (with the current position taken into account) */
  startedAt?: number;
  endsAt?: number;
  paused?: boolean;
}

type Status = "off" | "connecting" | "connected" | "no-discord";

const OP_HANDSHAKE = 0;
const OP_FRAME = 1;
const OP_CLOSE = 2;

export function encodeFrame(op: number, payload: unknown): Buffer {
  const json = Buffer.from(JSON.stringify(payload), "utf8");
  const header = Buffer.alloc(8);
  header.writeInt32LE(op, 0);
  header.writeInt32LE(json.length, 4);
  return Buffer.concat([header, json]);
}

/** split a byte stream into frames; returns the frames and what is left over */
export function decodeFrames(buffer: Buffer): { frames: Array<{ op: number; data: unknown }>; rest: Buffer } {
  const frames: Array<{ op: number; data: unknown }> = [];
  let offset = 0;
  while (buffer.length - offset >= 8) {
    const op = buffer.readInt32LE(offset);
    const length = buffer.readInt32LE(offset + 4);
    if (buffer.length - offset - 8 < length) break;
    const body = buffer.subarray(offset + 8, offset + 8 + length).toString("utf8");
    let data: unknown = null;
    try {
      data = JSON.parse(body);
    } catch {
      // ignore a broken frame
    }
    frames.push({ op, data });
    offset += 8 + length;
  }
  return { frames, rest: buffer.subarray(offset) };
}

function pipePath(index: number): string {
  if (process.platform === "win32") return `\\\\?\\pipe\\discord-ipc-${index}`;
  const base = process.env.XDG_RUNTIME_DIR || process.env.TMPDIR || process.env.TMP || process.env.TEMP || os.tmpdir();
  return path.join(base, `discord-ipc-${index}`);
}

export function activityFor(state: PresenceState) {
  const text = (value: string) => (value.length < 2 ? `${value}  ` : value.slice(0, 127));
  return {
    type: 2, // "Listening to"
    details: text(state.title),
    state: text(state.artist || "EvPlayer"),
    ...(state.paused || !state.startedAt ? {} : { timestamps: { start: state.startedAt, ...(state.endsAt ? { end: state.endsAt } : {}) } }),
    assets: {
      ...(state.artwork && /^https:/.test(state.artwork) ? { large_image: state.artwork } : {}),
      large_text: text(state.title),
    },
    ...(state.url && /^https:/.test(state.url) ? { buttons: [{ label: "Слушать", url: state.url }] } : {}),
  };
}

export class DiscordPresence {
  private appId: string | null = null;
  private socket: net.Socket | null = null;
  private ready = false;
  private state: PresenceState | null = null;
  private retry: ReturnType<typeof setTimeout> | null = null;
  private current: Status = "off";
  private nonce = 0;

  status(): Status {
    return this.current;
  }

  setAppId(appId: string | null): void {
    const clean = appId && /^\d{15,22}$/.test(appId.trim()) ? appId.trim() : null;
    if (clean === this.appId) return;
    this.appId = clean;
    this.disconnect();
    if (clean) this.connect(0);
  }

  update(state: PresenceState | null): void {
    this.state = state;
    this.send();
  }

  destroy(): void {
    this.appId = null;
    this.disconnect();
  }

  private disconnect(): void {
    if (this.retry) clearTimeout(this.retry);
    this.retry = null;
    this.ready = false;
    this.socket?.destroy();
    this.socket = null;
    this.current = this.appId ? "connecting" : "off";
  }

  private connect(index: number): void {
    if (!this.appId) return;
    this.current = "connecting";
    const socket = net.createConnection(pipePath(index));
    let buffer: Buffer = Buffer.alloc(0);
    socket.once("connect", () => {
      this.socket = socket;
      socket.write(encodeFrame(OP_HANDSHAKE, { v: 1, client_id: this.appId }));
    });
    socket.on("data", (chunk: Buffer) => {
      const { frames, rest } = decodeFrames(Buffer.concat([buffer, chunk]));
      buffer = rest;
      for (const frame of frames) {
        const data = frame.data as { evt?: string } | null;
        if (frame.op === OP_FRAME && data?.evt === "READY") {
          this.ready = true;
          this.current = "connected";
          this.send();
        }
        if (frame.op === OP_CLOSE) socket.destroy();
      }
    });
    socket.on("error", () => {
      socket.destroy();
      if (this.socket !== socket && index < 9) {
        this.connect(index + 1);
        return;
      }
    });
    socket.on("close", () => {
      if (this.socket === socket || this.socket === null) {
        this.socket = null;
        this.ready = false;
        if (!this.appId) return;
        this.current = "no-discord";
        // Discord may be started later
        if (this.retry) clearTimeout(this.retry);
        this.retry = setTimeout(() => this.connect(0), 20_000);
      }
    });
  }

  private send(): void {
    if (!this.socket || !this.ready) return;
    this.socket.write(
      encodeFrame(OP_FRAME, {
        cmd: "SET_ACTIVITY",
        args: { pid: process.pid, activity: this.state ? activityFor(this.state) : null },
        nonce: String(++this.nonce),
      }),
    );
  }
}
