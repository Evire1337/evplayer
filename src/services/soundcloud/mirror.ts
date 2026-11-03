import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import { useSettings } from "@/settings/store";
import { useLibrary, type Playlist } from "@/library/store";
import type { Track } from "@/types";
import { ApiError, AuthError, call } from "./oauth";

// Full sync: what you like and which playlists you make in EvPlayer is mirrored to your
// SoundCloud account, and removed there when you remove it here.
//
// It works by comparison, not by recording clicks. `useMirror` remembers what was last
// written to SoundCloud (the "baseline"); every pass compares the library with it and sends
// only the difference. A pass that stops halfway (offline, rate limit) simply continues
// from where the baseline says it got to.
//  - likes: only SoundCloud songs (sc:ID). SoundCloud has no manual order for likes, so
//    moving a like around here changes nothing there.
//  - playlists: the player is the source of truth for the ones it made, including the order
//    of songs. Playlists imported from SoundCloud are left alone, and so is anything on
//    SoundCloud that EvPlayer did not create.

export interface Baseline {
  likes: string[];
  playlists: Record<string, { scId: string; signature: string }>;
}

export type Step =
  | { kind: "like"; track: string }
  | { kind: "unlike"; track: string }
  | { kind: "create"; playlist: string }
  | { kind: "update"; playlist: string; scId: string }
  | { kind: "drop"; playlist: string; scId: string };

const SC_TRACK = /^sc:(\d+)$/;

export const scTrackId = (id: string): string | null => SC_TRACK.exec(id)?.[1] ?? null;

/** imported from SoundCloud: lives there already, not ours to create or delete */
export const isScImport = (p: Pick<Playlist, "source">): boolean => Boolean(p.source?.startsWith("scp:"));

export function syncable(playlists: Playlist[]): Playlist[] {
  return playlists.filter((p) => !isScImport(p));
}

export function trackIds(p: Playlist): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const item of p.items) {
    const id = scTrackId(item.track.id);
    if (id && !seen.has(id)) {
      seen.add(id);
      out.push(id);
    }
  }
  return out;
}

/** everything that has to match on SoundCloud: title, description and the ordered songs */
export function signatureOf(p: Playlist): string {
  return JSON.stringify([p.title, p.description ?? "", trackIds(p)]);
}

/** what must be sent so that SoundCloud matches the library; likes go oldest first */
export function plan(likes: { track: Track; addedAt: string }[], playlists: Playlist[], base: Baseline): Step[] {
  const steps: Step[] = [];
  const wanted = new Map<string, string>();
  for (const like of likes) {
    const id = scTrackId(like.track.id);
    if (id) wanted.set(id, like.addedAt);
  }
  const have = new Set(base.likes);
  [...wanted].filter(([id]) => !have.has(id)).sort((a, b) => a[1].localeCompare(b[1])).forEach(([track]) => steps.push({ kind: "like", track }));
  for (const track of base.likes) if (!wanted.has(track)) steps.push({ kind: "unlike", track });

  const mine = syncable(playlists);
  const ids = new Set(mine.map((p) => p.id));
  for (const p of mine) {
    const known = base.playlists[p.id];
    if (!known) steps.push({ kind: "create", playlist: p.id });
    else if (known.signature !== signatureOf(p)) steps.push({ kind: "update", playlist: p.id, scId: known.scId });
  }
  for (const [playlist, known] of Object.entries(base.playlists)) if (!ids.has(playlist)) steps.push({ kind: "drop", playlist, scId: known.scId });
  return steps;
}

interface MirrorState {
  base: Baseline;
  running: boolean;
  error: string | null;
  lastRun: string | null;
  patch: (p: Partial<Pick<MirrorState, "running" | "error" | "lastRun">>) => void;
  commit: (fn: (b: Baseline) => Baseline) => void;
  forget: () => void;
}

const EMPTY: Baseline = { likes: [], playlists: {} };

export const useMirror = create<MirrorState>()(
  persist(
    (set) => ({
      base: EMPTY,
      running: false,
      error: null,
      lastRun: null,
      patch: (p) => set(p),
      commit: (fn) => set((s) => ({ base: fn(s.base) })),
      forget: () => set({ base: EMPTY, error: null, lastRun: null }),
    }),
    { name: "evplayer.scmirror.v1", storage: createJSONStorage(() => localStorage), partialize: (s) => ({ base: s.base, lastRun: s.lastRun }) },
  ),
);

// ---- talking to SoundCloud --------------------------------------------------------------
// The guide writes songs and playlists as urns (soundcloud:tracks:1), the OpenAPI file as
// plain numbers. Urns go first; if SoundCloud refuses them the numbers are tried, and the
// style that worked is kept for the rest of the run.

let style: "urn" | "id" = "urn";

const ref = (kind: "tracks" | "playlists", id: string) => (style === "urn" ? `soundcloud:${kind}:${id}` : id);

async function either<T>(job: () => Promise<T>): Promise<T> {
  try {
    return await job();
  } catch (e) {
    if (!(e instanceof ApiError) || ![400, 404, 422].includes(e.status)) throw e;
    const other = style === "urn" ? "id" : "urn";
    const previous = style;
    style = other;
    try {
      return await job();
    } catch (second) {
      style = previous;
      throw second;
    }
  }
}

const playlistBody = (p: Playlist) => ({
  playlist: {
    title: p.title,
    description: p.description ?? "",
    sharing: "private",
    tracks: trackIds(p).map((id) => (style === "urn" ? { urn: `soundcloud:tracks:${id}` } : { id: Number(id) })),
  },
});

async function run(step: Step, byId: Map<string, Playlist>): Promise<void> {
  const { commit } = useMirror.getState();
  switch (step.kind) {
    case "like":
      await either(() => call("POST", `/likes/tracks/${ref("tracks", step.track)}`));
      commit((b) => ({ ...b, likes: [...new Set([...b.likes, step.track])] }));
      return;
    case "unlike":
      try {
        await either(() => call("DELETE", `/likes/tracks/${ref("tracks", step.track)}`));
      } catch (e) {
        // already gone from SoundCloud: nothing left to remove
        if (!(e instanceof ApiError) || e.status !== 404) throw e;
      }
      commit((b) => ({ ...b, likes: b.likes.filter((id) => id !== step.track) }));
      return;
    case "create": {
      const p = byId.get(step.playlist);
      if (!p) return;
      const signature = signatureOf(p);
      const made = await either(() => call<{ id?: number; urn?: string }>("POST", "/playlists", playlistBody(p)));
      const scId = made.id !== undefined ? String(made.id) : /(\d+)$/.exec(made.urn ?? "")?.[1];
      if (!scId) throw new Error("SoundCloud не вернул номер созданного плейлиста");
      commit((b) => ({ ...b, playlists: { ...b.playlists, [step.playlist]: { scId, signature } } }));
      return;
    }
    case "update": {
      const p = byId.get(step.playlist);
      if (!p) return;
      const signature = signatureOf(p);
      try {
        await either(() => call("PUT", `/playlists/${ref("playlists", step.scId)}`, playlistBody(p)));
      } catch (e) {
        // deleted on SoundCloud meanwhile: forget it, the next pass creates it again
        if (e instanceof ApiError && e.status === 404) {
          commit((b) => {
            const { [step.playlist]: _gone, ...rest } = b.playlists;
            return { ...b, playlists: rest };
          });
          return;
        }
        throw e;
      }
      commit((b) => ({ ...b, playlists: { ...b.playlists, [step.playlist]: { scId: step.scId, signature } } }));
      return;
    }
    case "drop":
      try {
        await either(() => call("DELETE", `/playlists/${ref("playlists", step.scId)}`));
      } catch (e) {
        if (!(e instanceof ApiError) || e.status !== 404) throw e;
      }
      commit((b) => {
        const { [step.playlist]: _gone, ...rest } = b.playlists;
        return { ...b, playlists: rest };
      });
  }
}

export function enabled(): boolean {
  const s = useSettings.getState();
  return s.soundcloudFullSync && Boolean(s.soundcloudLogin);
}

let again = false;

/** one pass: compares and sends the difference; passes asked for meanwhile are folded into one more */
export async function syncNow(pause = 400): Promise<void> {
  if (!enabled()) return;
  const state = useMirror.getState();
  if (state.running) {
    again = true;
    return;
  }
  state.patch({ running: true, error: null });
  try {
    do {
      again = false;
      const lib = useLibrary.getState();
      const steps = plan(lib.likes, lib.playlists, useMirror.getState().base);
      const byId = new Map(lib.playlists.map((p) => [p.id, p]));
      let failed: string | null = null;
      for (const step of steps) {
        if (!enabled()) return;
        try {
          await run(step, byId);
        } catch (e) {
          if (e instanceof AuthError) throw e;
          failed ??= e instanceof Error ? e.message : String(e);
        }
        await new Promise((r) => setTimeout(r, pause));
      }
      if (failed) useMirror.getState().patch({ error: failed });
    } while (again);
    useMirror.getState().patch({ lastRun: new Date().toISOString() });
  } catch (e) {
    useMirror.getState().patch({ error: e instanceof Error ? e.message : String(e) });
  } finally {
    useMirror.getState().patch({ running: false });
  }
}

let timer: ReturnType<typeof setTimeout> | undefined;

/** a few seconds after the last change in the library (or on start), run a pass */
export function startMirror(): void {
  const schedule = (ms: number) => {
    clearTimeout(timer);
    timer = setTimeout(() => void syncNow(), ms);
  };
  useLibrary.subscribe((s, prev) => {
    if (enabled() && (s.likes !== prev.likes || s.playlists !== prev.playlists)) schedule(4000);
  });
  useSettings.subscribe((s, prev) => {
    if (s.soundcloudFullSync && !prev.soundcloudFullSync) schedule(500);
  });
  if (enabled()) schedule(10_000);
}
