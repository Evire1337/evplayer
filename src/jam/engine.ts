import { player, type PlayerAction } from "@/player/engine";
import { currentTrack, usePlayer } from "@/player/store";
import type { Track } from "@/types";
import { toast } from "@/ui/toasts";
import type { JamLink } from "./link";
import { DRIFT_MS, fitTracks, newCode, normalizeCode, positionNow, queueWindow, sameName } from "./protocol";
import { addChat, addReaction, nameOf, noteAddedBy, resetJam, useJam } from "./store";
import { JAM_MAX, type JamControl, type JamMessage, type JamNow, type JamPerson, type JamRequest } from "./types";

// A listening session. The host's player is the one that counts: it plays as usual and
// tells everyone what it is doing a few times a second at most (and every few seconds
// anyway). Guests follow it, and what a guest presses goes to the host as a request, so
// there is one queue and one clock. Guests say they are still there every few seconds;
// whoever goes quiet (closed the app, lost the network) drops out of the list.

/** the host repeats where it is this often, even when nothing changes */
const BEAT_MS = 4000;
/** and the whole visible queue every this many beats (a guest may have missed a change) */
const FULL_EVERY = 6;
const HERE_MS = 10_000;
/** a guest not heard from for this long is gone */
const GUEST_GONE_MS = 40_000;
/** the host not heard from for this long: the session is over for this guest */
const HOST_GONE_MS = 30_000;
const JOIN_TIMEOUT_MS = 12_000;
/** time to get "I'm leaving" out before the connection closes */
const GOODBYE_MS = 1500;

interface Before {
  queue: Track[];
  index: number;
  positionMs: number;
}

const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));

class JamEngine {
  // the network part (and its cryptography) loads only when a session starts: most people
  // never open one, and it would cost them memory and start-up time
  private link: JamLink | null = null;
  private timers: Array<ReturnType<typeof setInterval>> = [];
  private stopWatching: (() => void) | null = null;
  // host
  private heard = new Map<string, number>();
  private banned = new Set<string>();
  private rev = 0;
  private sentRev = -1;
  private beats = 0;
  private sendTimer: ReturnType<typeof setTimeout> | undefined;
  // guest
  private hostId = "";
  private lastFromHost = 0;
  private offset = 0;
  private queueRev = -1;
  private before: Before | null = null;
  private joinTimer: ReturnType<typeof setTimeout> | undefined;
  private warnedLocal = "";
  /** the song this guest's player was told to play (the queue may already show the next one) */
  private playingId = "";

  constructor() {
    // closing the app: say goodbye, so the others do not wait for the timeout
    const bye = () => {
      const s = useJam.getState();
      if (s.status === "live") void this.link?.send(s.host ? { t: "end" } : { t: "leave" });
    };
    window.addEventListener("beforeunload", bye);
    // a phone closes the page without asking first
    window.addEventListener("pagehide", bye);
  }

  private async open(code: string): Promise<void> {
    if (!this.link) {
      const { JamLink } = await import("./link");
      this.link = new JamLink((msg, from) => this.receive(msg, from));
    }
    await this.link.join(code);
  }

  /** the link of a session that is on (it is open from start or join on) */
  private get wire(): JamLink {
    return this.link!;
  }

  get myId(): string {
    return this.link?.id ?? "";
  }

  // ---- starting and leaving -------------------------------------------------------------

  async start(): Promise<void> {
    const jam = useJam.getState();
    if (jam.status !== "idle" || !this.nameIsSet()) return;
    useJam.setState({ status: "starting" });
    const code = newCode();
    try {
      await this.open(code);
    } catch (e) {
      resetJam();
      toast(errorText(e), "error");
      return;
    }
    const me: JamPerson = { id: this.wire.id, name: jam.name.trim(), color: jam.color, host: true };
    useJam.setState({ status: "live", host: true, code, people: [me] });
    this.rev++;
    player.steerWith(this.steerHost);
    this.stopWatching = this.watchPlayer();
    this.timers.push(setInterval(() => this.beat(), BEAT_MS));
  }

  async join(input: string): Promise<void> {
    const jam = useJam.getState();
    if (jam.status !== "idle" || !this.nameIsSet()) return;
    const code = normalizeCode(input);
    if (!code) {
      toast("Это не похоже на код сессии: он выглядит как JAM-7K9P2Q", "error");
      return;
    }
    useJam.setState({ status: "joining", code });
    try {
      await this.open(code);
    } catch (e) {
      this.finish(errorText(e));
      return;
    }
    void this.wire.send({ t: "join", name: jam.name.trim(), color: jam.color });
    this.joinTimer = setTimeout(() => {
      if (useJam.getState().status === "joining") this.finish("Сессия не нашлась. Проверьте код: организатор должен быть в сети.");
    }, JOIN_TIMEOUT_MS);
  }

  /** leave (a guest) or end the session for everyone (the host) */
  async leave(): Promise<void> {
    const s = useJam.getState();
    if (s.status === "idle") return;
    if (s.status === "live") {
      const bye = this.wire.send(s.host ? { t: "end" } : { t: "leave" });
      await Promise.race([bye, new Promise((r) => setTimeout(r, GOODBYE_MS))]);
    }
    this.finish(s.status !== "live" ? undefined : s.host ? "Сессия завершена" : "Вы вышли из сессии");
  }

  private nameIsSet(): boolean {
    if (useJam.getState().name.trim()) return true;
    toast("Сначала напишите своё имя: так вас увидят в сессии", "error");
    return false;
  }

  /** stop everything here; a guest gets their own music back, paused */
  private finish(message?: string): void {
    const wasGuest = !useJam.getState().host && useJam.getState().status === "live";
    for (const t of this.timers) clearInterval(t);
    this.timers = [];
    clearTimeout(this.joinTimer);
    clearTimeout(this.sendTimer);
    this.stopWatching?.();
    this.stopWatching = null;
    this.link?.close();
    player.steerWith(null);
    if (wasGuest) this.giveBackPlayer();
    this.heard.clear();
    this.banned.clear();
    this.hostId = "";
    this.queueRev = -1;
    this.sentRev = -1;
    this.before = null;
    this.warnedLocal = "";
    this.playingId = "";
    resetJam();
    if (message) toast(message);
  }

  /** the session's music stops; what this guest was listening to before comes back */
  private giveBackPlayer(): void {
    if (usePlayer.getState().status === "playing") player.toggle();
    const before = this.before;
    if (before?.queue.length) player.restore(before.queue, before.index, before.positionMs);
    else usePlayer.setState({ status: "idle" });
  }

  // ---- the host --------------------------------------------------------------------------

  /** the host's own actions go through as usual; only who added a song is noted */
  private steerHost = (a: PlayerAction): boolean => {
    const me = useJam.getState().name;
    if (a.type === "add") noteAddedBy(a.track.id, me);
    else if (a.type === "playQueue" && a.tracks[a.startIndex]) noteAddedBy(a.tracks[a.startIndex].id, me);
    return false;
  };

  /** anything the others need to know about is sent soon after it happens */
  private watchPlayer(): () => void {
    return usePlayer.subscribe((s, prev) => {
      // the visible part of the queue follows the current song, so a new song sends it again
      if (s.queue !== prev.queue || s.index !== prev.index) this.rev++;
      const seeked = Math.abs(s.positionMs - prev.positionMs) > 2500;
      const playing = (st: string) => st === "playing" || st === "loading";
      if (s.queue !== prev.queue || s.index !== prev.index || playing(s.status) !== playing(prev.status) || seeked) this.sendSoon();
    });
  }

  private sendSoon(): void {
    clearTimeout(this.sendTimer);
    // a few changes in a row (the queue, then the index) go out as one message
    this.sendTimer = setTimeout(() => this.sendState(), 120);
  }

  private beat(): void {
    if (++this.beats % FULL_EVERY === 0) this.sentRev = -1;
    this.sendState();
    // guests who went quiet
    const now = Date.now();
    for (const p of useJam.getState().people) {
      if (!p.host && now - (this.heard.get(p.id) ?? 0) > GUEST_GONE_MS) this.drop(p.id, `${p.name} отключился`);
    }
  }

  private nowPlaying(): JamNow {
    const s = usePlayer.getState();
    return {
      index: s.index,
      trackId: currentTrack(s)?.id ?? null,
      playing: s.status === "playing" || s.status === "loading",
      positionMs: Math.round(s.positionMs),
      at: Date.now(),
    };
  }

  private sendState(): void {
    clearTimeout(this.sendTimer);
    const j = useJam.getState();
    if (!j.host || j.status !== "live") return;
    const s = usePlayer.getState();
    let queue;
    if (this.sentRev !== this.rev) {
      queue = queueWindow(s.queue, s.index, this.rev);
      queue.by = Object.fromEntries(queue.tracks.flatMap((t) => (j.addedBy[t.id] ? [[t.id, j.addedBy[t.id]]] : [])));
      this.sentRev = this.rev;
    }
    void this.wire.send({ t: "state", code: j.code, control: j.control, people: j.people, now: this.nowPlaying(), queue, rev: this.rev });
  }

  private drop(id: string, message?: string): void {
    this.heard.delete(id);
    useJam.setState((s) => ({ people: s.people.filter((p) => p.id !== id) }));
    if (message) toast(message);
    this.sendSoon();
  }

  private hostReceive(msg: JamMessage, from: string): void {
    const j = useJam.getState();
    const member = j.people.some((p) => p.id === from);
    if (member) this.heard.set(from, Date.now());
    switch (msg.t) {
      case "join": {
        const name = String(msg.name ?? "").trim().slice(0, 24);
        const refuse = (reason: string) => void this.wire.send({ t: "refuse", to: from, reason });
        if (this.banned.has(from)) return refuse("Организатор исключил вас из этой сессии");
        if (!name) return refuse("Напишите своё имя, чтобы войти");
        if (!member && j.people.length >= JAM_MAX) return refuse("В сессии уже нет мест");
        if (j.people.some((p) => p.id !== from && sameName(p.name, name))) return refuse(`Имя «${name}» в этой сессии уже занято. Выберите другое.`);
        const person: JamPerson = { id: from, name, color: typeof msg.color === "string" ? msg.color : "#6ff0c4", host: false };
        useJam.setState((s) => ({ people: [...s.people.filter((p) => p.id !== from), person] }));
        this.heard.set(from, Date.now());
        if (!member) toast(`${name} слушает вместе с вами`);
        // the newcomer needs the whole visible queue
        this.sentRev = -1;
        this.sendState();
        return;
      }
      case "leave":
        if (member) this.drop(from, `${nameOf(from)} вышел из сессии`);
        return;
      case "ask":
        if (member) this.apply(msg.req, from);
        return;
      case "chat":
      case "react":
        if (member) this.show(msg, from);
        return;
    }
  }

  /** a guest's request, done on the host's player (which then tells everyone) */
  private apply(req: JamRequest, from: string): void {
    const who = nameOf(from);
    const s = usePlayer.getState();
    // the host decides, but a song may always be added
    if (useJam.getState().control === "host" && req.do !== "add") return;
    const at = (index: number, trackId: string) => (s.queue[index]?.id === trackId ? index : s.queue.findIndex((t) => t.id === trackId));
    switch (req.do) {
      case "toggle":
        if (req.playing !== (s.status === "playing")) player.toggle();
        return;
      case "next":
        return player.next();
      case "prev":
        return player.prev();
      case "seek":
        return player.seek(req.ms);
      case "jump": {
        const i = at(req.index, req.trackId);
        if (i >= 0) void player.playIndex(i);
        return;
      }
      case "play": {
        const track = req.tracks[req.startIndex];
        if (!track) return;
        noteAddedBy(track.id, who);
        toast(`${who} включает «${track.title}»`);
        void player.playQueue(req.tracks, req.startIndex, null);
        return;
      }
      case "add":
        noteAddedBy(req.track.id, who);
        toast(`${who} добавляет «${req.track.title}»`);
        if (req.next) player.playNext(req.track);
        else player.addToQueue(req.track);
        return;
      case "remove": {
        const i = at(req.index, req.trackId);
        if (i >= 0) player.removeFromQueue(i);
        return;
      }
      case "move":
        return player.moveInQueue(req.from, req.to);
      case "clear":
        return player.clearUpcoming();
    }
  }

  // ---- the host's controls ---------------------------------------------------------------

  setControl(control: JamControl): void {
    if (!useJam.getState().host) return;
    useJam.setState({ control });
    this.sendSoon();
  }

  kick(id: string): void {
    const j = useJam.getState();
    const person = j.people.find((p) => p.id === id);
    if (!j.host || !person || person.host) return;
    this.banned.add(id);
    void this.wire.send({ t: "kick", id });
    this.drop(id, `${person.name} исключён из сессии`);
  }

  /**
   * A new code (and so a new link and QR code). The people in the session move along on
   * their own; the old code, and whoever only knows it, lead nowhere any more.
   */
  async changeCode(): Promise<void> {
    const j = useJam.getState();
    if (!j.host || j.status !== "live") return;
    const code = newCode();
    await Promise.all(j.people.filter((p) => !p.host).map((p) => this.wire.send({ t: "moved", to: p.id, sealed: this.wire.seal(code, p.id) })));
    try {
      await this.wire.join(code);
    } catch (e) {
      toast(errorText(e), "error");
      return;
    }
    useJam.setState({ code });
    this.sentRev = -1;
    this.sendState();
    toast(`Новый код сессии: ${code}`);
  }

  // ---- a guest ---------------------------------------------------------------------------

  /** what a guest presses becomes a request to the host */
  private steerGuest = (a: PlayerAction): boolean => {
    const j = useJam.getState();
    const s = usePlayer.getState();
    const ask = (req: JamRequest) => void this.wire.send({ t: "ask", req });
    const hostIndex = (i: number) => i + this.offset;
    switch (a.type) {
      case "ended":
        return true; // the host's player moves on, and this one follows
      case "shuffle":
      case "repeat":
        toast("Перемешивание и повтор в сессии — у организатора");
        return true;
      case "add":
        noteAddedBy(a.track.id, j.name);
        ask({ do: "add", track: a.track, next: a.next });
        toast(a.next ? `«${a.track.title}» сыграет следующим` : `«${a.track.title}» добавлен в общую очередь`);
        return true;
    }
    if (j.control === "host") {
      // a list pressed: its song goes into the queue instead
      const track = a.type === "playQueue" ? a.tracks[a.startIndex] : undefined;
      if (track) {
        noteAddedBy(track.id, j.name);
        ask({ do: "add", track, next: false });
        toast(`Музыкой управляет организатор: «${track.title}» добавлен в очередь`);
      } else {
        toast("Музыкой в этой сессии управляет организатор");
      }
      return true;
    }
    switch (a.type) {
      case "toggle":
        ask({ do: "toggle", playing: s.status !== "playing" });
        break;
      case "next":
      case "prev":
        ask({ do: a.type });
        break;
      case "seek":
        ask({ do: "seek", ms: a.ms });
        break;
      case "playIndex": {
        const track = s.queue[a.index];
        if (track) ask({ do: "jump", index: hostIndex(a.index), trackId: track.id });
        break;
      }
      case "playQueue": {
        const fit = fitTracks(a.tracks, a.startIndex);
        const track = fit.tracks[fit.startIndex];
        if (!track) break;
        noteAddedBy(track.id, j.name);
        ask({ do: "play", ...fit });
        break;
      }
      case "remove": {
        const track = s.queue[a.index];
        if (track) ask({ do: "remove", index: hostIndex(a.index), trackId: track.id });
        break;
      }
      case "move":
        ask({ do: "move", from: hostIndex(a.from), to: hostIndex(a.to) });
        break;
      case "clearUpcoming":
        ask({ do: "clear" });
        break;
    }
    return true;
  };

  private guestReceive(msg: JamMessage, from: string): void {
    const j = useJam.getState();
    const fromHost = from === this.hostId;
    if (fromHost) this.lastFromHost = Date.now();
    switch (msg.t) {
      case "state":
        return this.followState(msg, from);
      case "refuse":
        if (msg.to === this.wire.id && j.status === "joining") this.finish(msg.reason);
        return;
      case "kick":
        if (fromHost && msg.id === this.wire.id) this.finish("Организатор исключил вас из сессии");
        return;
      case "moved": {
        if (!fromHost || msg.to !== this.wire.id) return;
        const code = normalizeCode(this.wire.unseal(msg.sealed, from) ?? "");
        if (code) {
          useJam.setState({ code });
          void this.wire.join(code).catch((e) => this.finish(errorText(e)));
        }
        return;
      }
      case "end":
        if (fromHost) this.finish("Организатор завершил сессию");
        return;
      case "chat":
      case "react":
        if (j.people.some((p) => p.id === from)) this.show(msg, from);
        return;
    }
  }

  private followState(msg: Extract<JamMessage, { t: "state" }>, from: string): void {
    const j = useJam.getState();
    // only the host speaks for the session: the key that is marked as the host in it
    if (this.hostId ? from !== this.hostId : !msg.people.some((p) => p.id === from && p.host)) return;
    const inside = msg.people.some((p) => p.id === this.wire.id);
    if (j.status === "joining") {
      if (!inside) return; // the host has not let us in yet
      this.welcome(from);
    } else if (!inside) {
      this.finish("Вы больше не в этой сессии");
      return;
    }
    this.lastFromHost = Date.now();
    useJam.setState({ people: msg.people, control: msg.control, code: msg.code });
    if (msg.queue) {
      this.offset = msg.queue.offset;
      this.queueRev = msg.queue.rev;
      if (msg.queue.by) useJam.setState((s) => ({ addedBy: { ...s.addedBy, ...msg.queue!.by } }));
      usePlayer.setState({ queue: msg.queue.tracks, index: msg.now.index - this.offset });
    }
    // the queue changed and its new part has not come yet: the positions would not match
    if (msg.rev !== this.queueRev) return;
    this.follow(msg.now);
  }

  private welcome(hostId: string): void {
    clearTimeout(this.joinTimer);
    this.hostId = hostId;
    const s = usePlayer.getState();
    this.before = { queue: s.queue, index: s.index, positionMs: s.positionMs };
    useJam.setState({ status: "live", host: false });
    player.steerWith(this.steerGuest);
    this.timers.push(
      setInterval(() => void this.wire.send({ t: "here" }), HERE_MS),
      setInterval(() => {
        if (Date.now() - this.lastFromHost > HOST_GONE_MS) this.finish("Связь с организатором пропала, сессия закрыта");
      }, 5000),
    );
    toast(`Вы слушаете вместе с ${nameOf(hostId)}`);
  }

  /** play what the host plays, where the host is */
  private follow(now: JamNow): void {
    const s = usePlayer.getState();
    const local = now.index - this.offset;
    const track = s.queue[local];
    if (!track || track.id !== now.trackId) return;
    if (track.source === "local") {
      if (this.warnedLocal !== track.id) toast(`У организатора играет файл с его компьютера: «${track.title}». Здесь его не послушать.`);
      this.warnedLocal = track.id;
      return;
    }
    const target = positionNow(now, Date.now(), track.durationMs);
    player.unsteered(() => {
      if (this.playingId !== track.id || s.status === "idle" || s.status === "error") {
        this.playingId = track.id;
        if (!now.playing) {
          if (s.status === "playing") player.toggle();
          player.restore(s.queue, local, target);
          return;
        }
        void player.playIndex(local).then(() => {
          // the song took a moment to start: catch up with the host
          const behind = positionNow(now, Date.now(), track.durationMs);
          if (behind > DRIFT_MS) player.unsteered(() => player.seek(behind));
        });
        return;
      }
      if (s.status === "loading") return;
      if (now.playing !== (s.status === "playing")) player.toggle();
      if (Math.abs(s.positionMs - target) > DRIFT_MS) player.seek(target);
    });
  }

  // ---- both ------------------------------------------------------------------------------

  private receive(msg: JamMessage, from: string): void {
    const j = useJam.getState();
    if (j.status === "idle" || j.status === "starting") return;
    if (j.host) this.hostReceive(msg, from);
    else this.guestReceive(msg, from);
  }

  private show(msg: Extract<JamMessage, { t: "chat" | "react" }>, from: string): void {
    const name = nameOf(from);
    if (msg.t === "chat") addChat({ id: String(msg.id), from: name, text: String(msg.text).slice(0, 200), at: Date.now() });
    else addReaction({ id: String(msg.id), from: name, emoji: String(msg.emoji).slice(0, 8), at: Date.now() });
  }

  sendChat(text: string): void {
    const clean = text.trim().slice(0, 200);
    if (!clean || useJam.getState().status !== "live") return;
    const id = crypto.randomUUID();
    addChat({ id, from: useJam.getState().name, text: clean, at: Date.now() });
    void this.wire.send({ t: "chat", id, text: clean });
  }

  react(emoji: string): void {
    if (useJam.getState().status !== "live") return;
    const id = crypto.randomUUID();
    addReaction({ id, from: useJam.getState().name, emoji, at: Date.now() });
    void this.wire.send({ t: "react", id, emoji });
  }
}

export const jam = new JamEngine();
