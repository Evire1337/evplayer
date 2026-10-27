import { createHash } from "node:crypto";
import { createReadStream, promises as fs, watch, type FSWatcher } from "node:fs";
import path from "node:path";
import { Readable } from "node:stream";
import { parseFile } from "music-metadata";

// The user's own audio files. The library index (paths + tags) lives in userData; the audio
// itself is never copied, it is streamed straight from disk through the evplayer-media://
// scheme with range support so seeking works.

export const LOCAL_SCHEME = "evplayer-media";

export const AUDIO_EXTENSIONS = new Set([".mp3", ".flac", ".m4a", ".mp4", ".aac", ".ogg", ".oga", ".opus", ".wav", ".webm"]);

const MIME: Record<string, string> = {
  ".mp3": "audio/mpeg",
  ".flac": "audio/flac",
  ".m4a": "audio/mp4",
  ".mp4": "audio/mp4",
  ".aac": "audio/aac",
  ".ogg": "audio/ogg",
  ".oga": "audio/ogg",
  ".opus": "audio/ogg",
  ".wav": "audio/wav",
  ".webm": "audio/webm",
  ".jpg": "image/jpeg",
  ".png": "image/png",
};

export interface LocalTrackRecord {
  id: string;
  path: string;
  title: string;
  artists: string[];
  album?: string;
  durationMs: number;
  codec?: string;
  lossless?: boolean;
  sampleRate?: number;
  bitsPerSample?: number;
  bitrate?: number;
  /** Chromium cannot decode it (ALAC, for example) */
  unsupported?: boolean;
  hasCover?: boolean;
  /** tags were changed in EvPlayer: rescans keep them instead of re-reading the file */
  edited?: boolean;
  size: number;
  addedAt: string;
}

export interface LocalTrackEdit {
  title?: string;
  artists?: string[];
  album?: string | null;
  /** new cover as a data: url, or null to drop the cover */
  cover?: string | null;
}

interface IndexFile {
  version: 1;
  tracks: LocalTrackRecord[];
  /** folders added as a whole: new files in them are picked up automatically */
  folders?: string[];
}

export function localTrackId(filePath: string): string {
  return `local:${createHash("sha1").update(path.resolve(filePath).toLowerCase()).digest("hex").slice(0, 20)}`;
}

function titleFromFileName(filePath: string): { title: string; artist?: string } {
  const base = path.basename(filePath, path.extname(filePath)).replace(/_/g, " ").trim();
  const split = /^(?:\d{1,3}[\s.-]+)?(.+?)\s+-\s+(.+)$/.exec(base);
  return split ? { artist: split[1].trim(), title: split[2].trim() } : { title: base.replace(/^\d{1,3}[\s.-]+/, "") || base };
}

export class LocalLibrary {
  private tracks = new Map<string, LocalTrackRecord>();
  private folders = new Set<string>();
  private watchers = new Map<string, FSWatcher>();
  private rescanTimers = new Map<string, ReturnType<typeof setTimeout>>();
  private onChange: (() => void) | null = null;
  private loaded = false;

  constructor(private readonly dataDir: string) {}

  private get indexPath(): string {
    return path.join(this.dataDir, "local-tracks.json");
  }

  private get coverDir(): string {
    return path.join(this.dataDir, "local-covers");
  }

  private async load(): Promise<void> {
    if (this.loaded) return;
    this.loaded = true;
    try {
      const parsed = JSON.parse(await fs.readFile(this.indexPath, "utf8")) as IndexFile;
      for (const track of parsed.tracks ?? []) this.tracks.set(track.id, track);
      for (const folder of parsed.folders ?? []) this.folders.add(folder);
    } catch {
      // first run or unreadable index: start empty
    }
  }

  private async save(): Promise<void> {
    await fs.mkdir(this.dataDir, { recursive: true });
    const body: IndexFile = { version: 1, tracks: [...this.tracks.values()], folders: [...this.folders] };
    const tmp = `${this.indexPath}.tmp`;
    await fs.writeFile(tmp, JSON.stringify(body));
    await fs.rename(tmp, this.indexPath);
  }

  async list(): Promise<LocalTrackRecord[]> {
    await this.load();
    return [...this.tracks.values()].sort((a, b) => b.addedAt.localeCompare(a.addedAt));
  }

  async get(id: string): Promise<LocalTrackRecord | undefined> {
    await this.load();
    return this.tracks.get(id);
  }

  /** read tags of one file into a record; never throws for a bad file, it is just skipped */
  async readFile(filePath: string): Promise<LocalTrackRecord | null> {
    const ext = path.extname(filePath).toLowerCase();
    if (!AUDIO_EXTENSIONS.has(ext)) return null;
    let stat;
    try {
      stat = await fs.stat(filePath);
    } catch {
      return null;
    }
    if (!stat.isFile()) return null;
    const id = localTrackId(filePath);
    const fallback = titleFromFileName(filePath);
    try {
      const meta = await parseFile(filePath, { duration: true, skipCovers: false });
      const common = meta.common;
      const format = meta.format;
      const picture = common.picture?.[0];
      let hasCover = false;
      if (picture?.data?.length) {
        await fs.mkdir(this.coverDir, { recursive: true });
        const coverExt = /png/i.test(picture.format) ? ".png" : ".jpg";
        await fs.writeFile(path.join(this.coverDir, `${id.slice(6)}${coverExt}`), picture.data);
        hasCover = true;
      }
      const codec = format.codec ?? format.container;
      const artists = common.artists?.length ? common.artists : common.artist ? [common.artist] : fallback.artist ? [fallback.artist] : [];
      return {
        id,
        path: filePath,
        title: common.title?.trim() || fallback.title,
        artists,
        ...(common.album ? { album: common.album } : {}),
        durationMs: Math.round((format.duration ?? 0) * 1000),
        ...(codec ? { codec } : {}),
        ...(format.lossless !== undefined ? { lossless: format.lossless } : {}),
        ...(format.sampleRate ? { sampleRate: format.sampleRate } : {}),
        ...(format.bitsPerSample ? { bitsPerSample: format.bitsPerSample } : {}),
        ...(format.bitrate ? { bitrate: Math.round(format.bitrate) } : {}),
        ...(/alac/i.test(codec ?? "") ? { unsupported: true } : {}),
        hasCover,
        size: stat.size,
        addedAt: new Date().toISOString(),
      };
    } catch {
      return {
        id,
        path: filePath,
        title: fallback.title,
        artists: fallback.artist ? [fallback.artist] : [],
        durationMs: 0,
        size: stat.size,
        addedAt: new Date().toISOString(),
      };
    }
  }

  async collectAudioFiles(entries: string[], depth = 0): Promise<string[]> {
    const out: string[] = [];
    for (const entry of entries) {
      let stat;
      try {
        stat = await fs.stat(entry);
      } catch {
        continue;
      }
      if (stat.isDirectory()) {
        if (depth > 8) continue;
        const children = await fs.readdir(entry).catch(() => [] as string[]);
        out.push(...(await this.collectAudioFiles(children.map((name) => path.join(entry, name)), depth + 1)));
      } else if (AUDIO_EXTENSIONS.has(path.extname(entry).toLowerCase())) {
        out.push(entry);
      }
    }
    return out;
  }

  async add(paths: string[]): Promise<{ added: LocalTrackRecord[]; skipped: number }> {
    await this.load();
    for (const entry of paths) {
      const stat = await fs.stat(entry).catch(() => null);
      if (stat?.isDirectory() && !this.folders.has(path.resolve(entry))) {
        this.folders.add(path.resolve(entry));
        this.watchFolder(path.resolve(entry));
      }
    }
    const files = await this.collectAudioFiles(paths);
    const added: LocalTrackRecord[] = [];
    let skipped = 0;
    for (const file of files) {
      const existing = this.tracks.get(localTrackId(file));
      const record = await this.readFile(file);
      if (!record) {
        skipped++;
        continue;
      }
      if (existing) {
        record.addedAt = existing.addedAt;
        if (existing.edited) {
          // keep what the user typed in over the tags in the file
          record.title = existing.title;
          record.artists = existing.artists;
          if (existing.album) record.album = existing.album;
          else delete record.album;
          record.hasCover = existing.hasCover;
          record.edited = true;
        }
      }
      this.tracks.set(record.id, record);
      added.push(record);
    }
    if (added.length) await this.save();
    return { added, skipped };
  }

  async update(id: string, patch: LocalTrackEdit): Promise<LocalTrackRecord | undefined> {
    await this.load();
    const record = this.tracks.get(id);
    if (!record) return undefined;
    if (typeof patch.title === "string" && patch.title.trim()) record.title = patch.title.trim().slice(0, 300);
    if (Array.isArray(patch.artists)) record.artists = patch.artists.map((a) => String(a).trim()).filter(Boolean).slice(0, 20);
    if (patch.album !== undefined) {
      if (patch.album && patch.album.trim()) record.album = patch.album.trim().slice(0, 300);
      else delete record.album;
    }
    if (patch.cover !== undefined) {
      const stem = id.slice(6);
      await fs.rm(path.join(this.coverDir, `${stem}.jpg`), { force: true }).catch(() => {});
      await fs.rm(path.join(this.coverDir, `${stem}.png`), { force: true }).catch(() => {});
      const match = patch.cover ? /^data:image\/(png|jpe?g|webp);base64,(.+)$/i.exec(patch.cover) : null;
      if (match) {
        await fs.mkdir(this.coverDir, { recursive: true });
        await fs.writeFile(path.join(this.coverDir, `${stem}${match[1].toLowerCase() === "png" ? ".png" : ".jpg"}`), Buffer.from(match[2], "base64"));
        record.hasCover = true;
      } else {
        record.hasCover = false;
      }
    }
    record.edited = true;
    await this.save();
    return record;
  }

  async watchedFolders(): Promise<string[]> {
    await this.load();
    return [...this.folders];
  }

  async unwatchFolder(folder: string): Promise<void> {
    await this.load();
    this.folders.delete(folder);
    this.watchers.get(folder)?.close();
    this.watchers.delete(folder);
    await this.save();
  }

  /** catch up with changes made while the app was closed, then keep watching */
  async startWatching(onChange: () => void): Promise<void> {
    this.onChange = onChange;
    await this.load();
    for (const folder of this.folders) {
      await this.rescan(folder);
      this.watchFolder(folder);
    }
  }

  stopWatching(): void {
    for (const watcher of this.watchers.values()) watcher.close();
    this.watchers.clear();
    for (const timer of this.rescanTimers.values()) clearTimeout(timer);
    this.rescanTimers.clear();
  }

  private watchFolder(folder: string): void {
    if (!this.onChange || this.watchers.has(folder)) return;
    try {
      const watcher = watch(folder, { recursive: true }, () => this.scheduleRescan(folder));
      watcher.on("error", () => {
        watcher.close();
        this.watchers.delete(folder);
      });
      this.watchers.set(folder, watcher);
    } catch {
      // the folder is gone or cannot be watched (network drive): it is rescanned on next start
    }
  }

  private scheduleRescan(folder: string): void {
    clearTimeout(this.rescanTimers.get(folder));
    // files are often copied in bursts: wait for things to settle
    this.rescanTimers.set(
      folder,
      setTimeout(() => {
        this.rescanTimers.delete(folder);
        void this.rescan(folder).then((changed) => changed && this.onChange?.());
      }, 2000),
    );
  }

  /** add files that appeared in the folder and drop ones that were deleted; true if anything changed */
  async rescan(folder: string): Promise<boolean> {
    const files = new Set((await this.collectAudioFiles([folder])).map((f) => path.resolve(f)));
    let changed = false;
    const prefix = path.resolve(folder).toLowerCase() + path.sep;
    for (const record of [...this.tracks.values()]) {
      const resolved = path.resolve(record.path);
      if (resolved.toLowerCase().startsWith(prefix) && !files.has(resolved)) {
        this.tracks.delete(record.id);
        changed = true;
      }
    }
    for (const file of files) {
      if (this.tracks.has(localTrackId(file))) continue;
      const record = await this.readFile(file);
      if (record) {
        this.tracks.set(record.id, record);
        changed = true;
      }
    }
    if (changed) await this.save();
    return changed;
  }

  async remove(ids: string[]): Promise<void> {
    await this.load();
    for (const id of ids) {
      this.tracks.delete(id);
      await fs.rm(path.join(this.coverDir, `${id.slice(6)}.jpg`), { force: true }).catch(() => {});
      await fs.rm(path.join(this.coverDir, `${id.slice(6)}.png`), { force: true }).catch(() => {});
    }
    await this.save();
  }

  /** answers evplayer-media://track/<id> and evplayer-media://cover/<id> */
  async handle(request: Request): Promise<Response> {
    const url = new URL(request.url);
    const kind = url.hostname;
    const id = decodeURIComponent(url.pathname.replace(/^\/+/, ""));
    const cors = { "access-control-allow-origin": "*", "access-control-expose-headers": "content-length, content-range, accept-ranges" };
    if (kind === "cover") {
      for (const ext of [".jpg", ".png"]) {
        const file = path.join(this.coverDir, `${id.replace(/^local:/, "")}${ext}`);
        try {
          const data = await fs.readFile(file);
          return new Response(data, { headers: { ...cors, "content-type": MIME[ext], "cache-control": "max-age=31536000" } });
        } catch {
          // try the next extension
        }
      }
      return new Response("not found", { status: 404, headers: cors });
    }
    if (kind !== "track") return new Response("not found", { status: 404, headers: cors });
    const record = await this.get(id);
    if (!record) return new Response("not found", { status: 404, headers: cors });
    let stat;
    try {
      stat = await fs.stat(record.path);
    } catch {
      return new Response("file moved or deleted", { status: 404, headers: cors });
    }
    const type = MIME[path.extname(record.path).toLowerCase()] ?? "application/octet-stream";
    const range = /bytes=(\d*)-(\d*)/.exec(request.headers.get("range") ?? "");
    if (range && (range[1] || range[2])) {
      let start = range[1] ? parseInt(range[1], 10) : stat.size - parseInt(range[2], 10);
      let end = range[1] && range[2] ? parseInt(range[2], 10) : stat.size - 1;
      start = Math.max(0, start);
      end = Math.min(stat.size - 1, end);
      if (start > end) {
        return new Response(null, { status: 416, headers: { ...cors, "content-range": `bytes */${stat.size}` } });
      }
      const stream = Readable.toWeb(createReadStream(record.path, { start, end })) as ReadableStream;
      return new Response(stream, {
        status: 206,
        headers: {
          ...cors,
          "content-type": type,
          "accept-ranges": "bytes",
          "content-length": String(end - start + 1),
          "content-range": `bytes ${start}-${end}/${stat.size}`,
        },
      });
    }
    const stream = Readable.toWeb(createReadStream(record.path)) as ReadableStream;
    return new Response(stream, {
      headers: { ...cors, "content-type": type, "accept-ranges": "bytes", "content-length": String(stat.size) },
    });
  }
}
