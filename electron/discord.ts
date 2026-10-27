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
  /** what Discord shows next to your name in the member list */
  statusShows?: "app" | "artist" | "song";
}

export interface DiscordUser {
  id: string;
  username: string;
  globalName?: string;
  avatar?: string;
}

type Status = "off" | "connecting" | "connected" | "no-discord" | "error";

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
    // 0: the application name, 1: the "state" line (artist), 2: the "details" line (song)
    status_display_type: state.statusShows === "artist" ? 1 : state.statusShows === "app" ? 0 : 2,
    details: text(state.title),
    state: text(state.artist || "EvPlayer"),
    // Discord rejects the whole activity if a timestamp is not a whole number
    ...(state.paused || !state.startedAt
      ? {}
      : { timestamps: { start: Math.round(state.startedAt), ...(state.endsAt ? { end: Math.round(state.endsAt) } : {}) } }),
    assets: {
      ...(state.artwork && /^https:/.test(state.artwork) ? { large_image: state.artwork } : {}),
      large_text: text(state.title),
    },
    ...(state.url && /^https:/.test(state.url) ? { buttons: [{ label: "Слушать", url: state.url }] } : {}),
  };
}

export class DiscordPresence {
  /** where the RPC pipe lives; overridable so tests can run their own mock server */
  private readonly ipcPath: (index: number) => string;
  private appId: string | null = null;
  private socket: net.Socket | null = null;
  private ready = false;
  private state: PresenceState | null = null;
  private retry: ReturnType<typeof setTimeout> | null = null;
  private current: Status = "off";
  private nonce = 0;
  private user: DiscordUser | null = null;
  private lastError: string | null = null;
  /** older Discord builds reject the newer "status_display_type" field */
  private plainActivity = false;
  private attempt = 0;

  constructor(ipcPath: (index: number) => string = pipePath) {
    this.ipcPath = ipcPath;
  }

  status(): { status: Status; user: DiscordUser | null; error: string | null } {
    return { status: this.current, user: this.current === "connected" ? this.user : null, error: this.lastError };
  }

  setAppId(appId: string | null): void {
    const clean = appId && /^\d{15,22}$/.test(appId.trim()) ? appId.trim() : null;
    if (clean === this.appId) return;
    this.appId = clean;
    this.lastError = null;
    this.disconnect();
    if (clean) void this.connect();
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
    this.attempt++;
    if (this.retry) clearTimeout(this.retry);
    this.retry = null;
    this.ready = false;
    this.socket?.removeAllListeners();
    this.socket?.destroy();
    this.socket = null;
    this.current = this.appId ? "connecting" : "off";
  }

  private scheduleRetry(): void {
    if (!this.appId) return;
    if (this.retry) clearTimeout(this.retry);
    // Discord may be started later
    this.retry = setTimeout(() => void this.connect(), 15_000);
  }

  /** the first of discord-ipc-0..9 that answers */
  private open(index = 0): Promise<net.Socket | null> {
    if (index > 9) return Promise.resolve(null);
    return new Promise((resolve) => {
      const socket = net.createConnection(this.ipcPath(index));
      socket.once("connect", () => {
        socket.removeAllListeners("error");
        resolve(socket);
      });
      socket.once("error", () => {
        socket.destroy();
        resolve(this.open(index + 1));
      });
    });
  }

  private async connect(): Promise<void> {
    if (!this.appId) return;
    const attempt = ++this.attempt;
    this.current = "connecting";
    const socket = await this.open();
    if (attempt !== this.attempt || !this.appId) {
      socket?.destroy();
      return;
    }
    if (!socket) {
      this.current = "no-discord";
      this.scheduleRetry();
      return;
    }
    this.socket = socket;
    let buffer: Buffer = Buffer.alloc(0);
    socket.on("data", (chunk: Buffer) => {
      const { frames, rest } = decodeFrames(Buffer.concat([buffer, chunk]));
      buffer = rest;
      for (const frame of frames) this.onFrame(frame.op, frame.data as Frame | null);
    });
    socket.on("error", () => socket.destroy());
    socket.on("close", () => {
      if (this.socket !== socket) return;
      this.socket = null;
      this.ready = false;
      if (this.current !== "error") this.current = "no-discord";
      this.scheduleRetry();
    });
    socket.write(encodeFrame(OP_HANDSHAKE, { v: 1, client_id: this.appId }));
  }

  private onFrame(op: number, data: Frame | null): void {
    if (op === OP_CLOSE) {
      // wrong application id and similar: Discord says why and hangs up
      this.lastError = data?.message ?? "Discord закрыл соединение";
      this.current = "error";
      this.socket?.destroy();
      return;
    }
    if (op !== OP_FRAME || !data) return;
    if (data.evt === "READY") {
      const u = data.data?.user;
      this.user = u?.id
        ? {
            id: u.id,
            username: u.username ?? "",
            globalName: u.global_name || undefined,
            avatar: u.avatar ? `https://cdn.discordapp.com/avatars/${u.id}/${u.avatar}.${u.avatar.startsWith("a_") ? "gif" : "png"}?size=128` : undefined,
          }
        : null;
      this.ready = true;
      this.current = "connected";
      this.lastError = null;
      this.send();
      return;
    }
    if (data.evt === "ERROR") {
      const message = data.data?.message ?? "Discord не принял статус";
      if (!this.plainActivity && /status_display_type/i.test(message)) {
        this.plainActivity = true;
        this.send();
        return;
      }
      this.lastError = message;
    } else if (data.cmd === "SET_ACTIVITY") {
      this.lastError = null;
    }
  }

  private send(): void {
    if (!this.socket || !this.ready) return;
    let activity: Record<string, unknown> | null = this.state ? activityFor(this.state) : null;
    if (activity && this.plainActivity) {
      const { status_display_type: _drop, ...rest } = activity;
      void _drop;
      activity = rest;
    }
    this.socket.write(encodeFrame(OP_FRAME, { cmd: "SET_ACTIVITY", args: { pid: process.pid, activity }, nonce: String(++this.nonce) }));
  }
}

interface Frame {
  evt?: string;
  cmd?: string;
  message?: string;
  data?: { message?: string; user?: { id?: string; username?: string; global_name?: string; avatar?: string } };
}
