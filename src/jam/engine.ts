import { player, type PlayerAction } from "@/player/engine";
import { currentTrack, usePlayer } from "@/player/store";
import type { Track } from "@/types";
import { toast } from "@/ui/toasts";
import type { JamLink } from "./link";
import { DRIFT_MS, fitTracks, LOBBY, newCode, normalizeCode, positionNow, queueWindow, sameName, ticketOf } from "./protocol";
import { addChat, addReaction, nameOf, noteAddedBy, resetJam, useJam } from "./store";
import { JAM_MAX, type JamAccess, type JamControl, type JamMessage, type JamNow, type JamPerson, type JamRequest } from "./types";

// A listening session. The host's player is the one that counts: it plays as usual and
// tells everyone what it is doing a few times a second at most (and every few seconds
// anyway). Guests follow it, and what a guest presses goes to the host as a request, so
// there is one queue and one clock. Guests say they are still there every few seconds;
// whoever goes quiet (closed the app, lost the network) drops out of the list.
//
// Who gets in is the host's call: anyone with the code, only whoever the host lets in, or
// nobody new. A session can also be put in the public list (a room everyone knows the key
// of), where others see it with the number of people in it.

/** the host repeats where it is this often, even when nothing changes */
const BEAT_MS = 4000;
/** and the whole visible queue every this many beats (a guest may have missed a change) */
const FULL_EVERY = 6;
/** a listed session says it is there every this many beats */
const ANNOUNCE_EVERY = 3;
const HERE_MS = 10_000;
/** a guest not heard from for this long is gone */
const GUEST_GONE_MS = 40_000;
/** the host not heard from for this long: the session is over for this guest */
const HOST_GONE_MS = 30_000;
const JOIN_TIMEOUT_MS = 12_000;
/** how long someone waits to be let in (and the host has to answer) */
const KNOCK_MS = 3 * 60_000;
/** a listed session not heard from for this long is taken off the list */
const LISTED_FOR_MS = 30_000;
/** time to get "I'm leaving" out before the connection closes */
const GOODBYE_MS = 1500;
/** a guest asks for the queue again at most this often */
const RESYNC_MS = 3000;
/** about how long a message takes from the host to a guest through the relays */
const LATENCY_MS = 150;
/** after a guest's own seek, the host's reports are not followed for this long */
const SEEK_HOLD_MS = 2500;

interface Before {
  queue: Track[];
  index: number;
  positionMs: number;
}

const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));
const ticket = () => Array.from(crypto.getRandomValues(new Uint8Array(12)), (b) => b.toString(16).padStart(2, "0")).join("");

class JamEngine {
  // the network part (and its cryptography) loads only when a session or the public list is
  // opened: most people never do, and it would cost them memory and start-up time
  private link: JamLink | null = null;
  private lobbyLink: JamLink | null = null;
  private browsing = false;
  private lobbyTimer: ReturnType<typeof setInterval> | undefined;
  private timers: Array<ReturnType<typeof setInterval>> = [];
  private stopWatching: (() => void) | null = null;
  // host
  private heard = new Map<string, number>();
  private banned = new Set<string>();
  /** a Discord invite carries this: whoever has it is let in without asking */
  private pass = "";
  private rev = 0;
  private sentRev = -1;
  private beats = 0;
  private sendTimer: ReturnType<typeof setTimeout> | undefined;
  // guest
  private hostId = "";
  private lastFromHost = 0;
  private offset = 0;
  private queueRev = -1;
  private lastResync = 0;
  private before: Before | null = null;
  private joinTimer: ReturnType<typeof setTimeout> | undefined;
  private warnedLocal = "";
  /** the song this guest's player was told to play (the queue may already show the next one) */
  private playingId = "";
  private seekHeldUntil = 0;

  constructor() {
    // closing the app: say goodbye, so the others do not wait for the timeout
    const bye = () => {
      const s = useJam.getState();
      if (s.status === "live" || s.waiting) void this.link?.send(s.host ? { t: "end" } : { t: "leave" });
      if (s.host && s.listed) void this.lobbyLink?.send({ t: "gone", code: s.code });
    };
    window.addEventListener("beforeunload", bye);
    // a phone closes the page without asking first
    window.addEventListener("pagehide", bye);
  }

  private async newLink(onMessage: (msg: JamMessage, from: string) => void): Promise<JamLink> {
    const { JamLink } = await import("./link");
    return new JamLink(onMessage);
  }

  private async open(code: string): Promise<void> {
    this.link ??= await this.newLink((msg, from) => this.receive(msg, from));
    await this.link.join(code);
  }

  /** the link of a session that is on (it is open from start or join on) */
  private get wire(): JamLink {
    return this.link!;
  }

  get myId(): string {
    return this.link?.id ?? "";
  }

  /** the pass for Discord invites (the host's, while the session is on) */
  get discordPass(): string {
    return useJam.getState().host ? this.pass : "";
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
    this.pass = ticket();
    useJam.setState({ status: "live", host: true, code, people: [me], ...jam.setup });
    this.rev++;
    player.steerWith(this.steerHost);
    this.stopWatching = this.watchPlayer();
    this.timers.push(setInterval(() => this.beat(), BEAT_MS));
    if (jam.setup.listed) void this.announce();
  }

  async join(input: string): Promise<void> {
    const jam = useJam.getState();
    if (jam.status !== "idle" || !this.nameIsSet()) return;
    const code = normalizeCode(input);
    if (!code) {
      toast("Это не похоже на код сессии: он выглядит как JAM-7K9P-2QXZ", "error");
      return;
    }
    useJam.setState({ status: "joining", code });
    try {
      await this.open(code);
    } catch (e) {
      this.finish(errorText(e));
      return;
    }
    void this.wire.send({ t: "join", name: jam.name.trim(), color: jam.color, ticket: ticketOf(input) });
    this.waitFor(JOIN_TIMEOUT_MS, "Сессия не нашлась. Проверьте код: организатор должен быть в сети.");
  }

  private waitFor(ms: number, message: string): void {
    clearTimeout(this.joinTimer);
    this.joinTimer = setTimeout(() => {
      if (useJam.getState().status === "joining") this.finish(message);
    }, ms);
  }

  /** leave (a guest), stop waiting to be let in, or end the session for everyone (the host) */
  async leave(): Promise<void> {
    const s = useJam.getState();
    if (s.status === "idle") return;
    if (s.status === "live" || s.waiting) {
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
    const s = useJam.getState();
    const wasGuest = !s.host && s.status === "live";
    if (s.host && s.listed) void this.unlist(s.code);
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
    this.pass = "";
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

  /** the host's own actions go through as usual; who added a song is noted and told */
  private steerHost = (a: PlayerAction): boolean => {
    const me = useJam.getState().name;
    if (a.type === "add") {
      noteAddedBy(a.track.id, me);
      this.tell(`${me} добавляет «${a.track.title}»`, this.wire.id);
    } else if (a.type === "playQueue" && a.tracks[a.startIndex]) {
      noteAddedBy(a.tracks[a.startIndex].id, me);
      this.tell(`${me} включает «${a.tracks[a.startIndex].title}»`, this.wire.id);
    }
    return false;
  };

  /** a line for everyone in the session (the one who did it already knows) */
  private tell(text: string, by?: string): void {
    void this.wire.send({ t: "note", text, by });
  }

  /** anything the others need to know about is sent soon after it happens */
  private watchPlayer(): () => void {
    return usePlayer.subscribe((s, prev) => {
      // the visible part of the queue follows the current song, so a new song sends it again
      if (s.queue !== prev.queue || s.index !== prev.index) this.rev++;
      const seeked = Math.abs(s.positionMs - prev.positionMs) > 2500;
      if (s.queue !== prev.queue || s.index !== prev.index || s.status !== prev.status || seeked) this.sendSoon();
    });
  }

  private sendSoon(): void {
    clearTimeout(this.sendTimer);
    // a few changes in a row (the queue, then the index) go out as one message
    this.sendTimer = setTimeout(() => this.sendState(), 120);
  }

  private beat(): void {
    this.beats++;
    if (this.beats % FULL_EVERY === 0) this.sentRev = -1;
    this.sendState();
    const now = Date.now();
    const j = useJam.getState();
    // guests who went quiet, and people who waited too long to be let in
    for (const p of j.people) {
      if (!p.host && now - (this.heard.get(p.id) ?? 0) > GUEST_GONE_MS) this.drop(p.id, `${p.name} отключился`);
    }
    if (j.knocks.some((k) => now - k.at > KNOCK_MS)) useJam.setState((s) => ({ knocks: s.knocks.filter((k) => now - k.at <= KNOCK_MS) }));
    if (j.listed && this.beats % ANNOUNCE_EVERY === 0) void this.announce();
  }

  private nowPlaying(): JamNow {
    const s = usePlayer.getState();
    return {
      index: s.index,
      trackId: currentTrack(s)?.id ?? null,
      // a song still loading is not playing yet: guests start it when the host's starts
      playing: s.status === "playing",
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
    void this.wire.send({ t: "state", code: j.code, control: j.control, access: j.access, people: j.people, now: this.nowPlaying(), queue, rev: this.rev });
  }

  private drop(id: string, message?: string): void {
    this.heard.delete(id);
    useJam.setState((s) => ({ people: s.people.filter((p) => p.id !== id) }));
    if (message) toast(message);
    this.sendSoon();
  }

  private refuse(to: string, reason: string): void {
    void this.wire.send({ t: "refuse", to, reason });
  }

  /** why this person cannot come in ("" when they can) */
  private keptOut(id: string, name: string): string {
    const j = useJam.getState();
    if (!name) return "Напишите своё имя, чтобы войти";
    if (j.people.length >= JAM_MAX) return "В сессии уже нет мест";
    if (j.people.some((p) => p.id !== id && sameName(p.name, name))) return `Имя «${name}» в этой сессии уже занято. Выберите другое.`;
    return "";
  }

  private hostReceive(msg: JamMessage, from: string): void {
    const j = useJam.getState();
    const member = j.people.some((p) => p.id === from);
    if (member) this.heard.set(from, Date.now());
    switch (msg.t) {
      case "join": {
        const name = String(msg.name ?? "").trim().slice(0, 24);
        const color = typeof msg.color === "string" ? msg.color : "#6ff0c4";
        const invited = Boolean(this.pass) && msg.ticket === this.pass;
        if (member) return this.letIn(from, name, color);
        if (this.banned.has(from) && !invited) return this.refuse(from, "Организатор исключил вас из этой сессии");
        const reason = this.keptOut(from, name);
        if (reason) return this.refuse(from, reason);
        if (!invited && j.access === "closed") return this.refuse(from, "Организатор закрыл вход в эту сессию");
        if (!invited && j.access === "ask") {
          useJam.setState((s) => ({ knocks: [...s.knocks.filter((k) => k.id !== from), { id: from, name, color, at: Date.now() }] }));
          void this.wire.send({ t: "wait", to: from });
          if (!j.knocks.some((k) => k.id === from)) toast(`${name} просится в сессию`);
          return;
        }
        return this.letIn(from, name, color);
      }
      case "leave":
        if (member) this.drop(from, `${nameOf(from)} вышел из сессии`);
        else useJam.setState((s) => ({ knocks: s.knocks.filter((k) => k.id !== from) }));
        return;
      case "resync":
        if (member) {
          this.sentRev = -1;
          this.sendSoon();
        }
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

  private letIn(id: string, name: string, color: string): void {
    const already = useJam.getState().people.some((p) => p.id === id);
    const person: JamPerson = { id, name, color, host: false };
    useJam.setState((s) => ({ people: [...s.people.filter((p) => p.id !== id), person], knocks: s.knocks.filter((k) => k.id !== id) }));
    this.heard.set(id, Date.now());
    if (!already) toast(`${name} слушает вместе с вами`);
    // the newcomer needs the whole visible queue
    this.sentRev = -1;
    this.sendState();
  }

  /** the host lets in someone who asked */
  admit(id: string): void {
    const knock = useJam.getState().knocks.find((k) => k.id === id);
    if (!knock) return;
    const reason = this.keptOut(id, knock.name);
    if (reason) {
      useJam.setState((s) => ({ knocks: s.knocks.filter((k) => k.id !== id) }));
      this.refuse(id, reason);
      toast(reason, "error");
      return;
    }
    this.letIn(id, knock.name, knock.color);
  }

  /** the host does not let someone in */
  turnAway(id: string): void {
    useJam.setState((s) => ({ knocks: s.knocks.filter((k) => k.id !== id) }));
    this.refuse(id, "Организатор не пустил вас в сессию");
  }

  /** a guest's request, done on the host's player (which then tells everyone) */
  private apply(req: JamRequest, from: string): void {
    const who = nameOf(from);
    const s = usePlayer.getState();
    // the host decides, but a song may always be added
    if (useJam.getState().control === "host" && req.do !== "add") return;
    const at = (index: number, trackId: string) => (s.queue[index]?.id === trackId ? index : s.queue.findIndex((t) => t.id === trackId));
    // the host's own steer must not take these as the host's doing
    player.unsteered(() => {
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
          this.tell(`${who} включает «${track.title}»`, from);
          void player.playQueue(req.tracks, req.startIndex, null);
          return;
        }
        case "add":
          noteAddedBy(req.track.id, who);
          toast(`${who} добавляет «${req.track.title}»`);
          this.tell(`${who} добавляет «${req.track.title}»`, from);
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
    });
  }

  // ---- the host's controls ---------------------------------------------------------------

  setControl(control: JamControl): void {
    const j = useJam.getState();
    j.setSetup({ control });
    if (!j.host || j.status !== "live") return;
    useJam.setState({ control });
    this.sendSoon();
  }

  /** who gets in from now on; closing turns away everyone still waiting */
  setAccess(access: JamAccess): void {
    const j = useJam.getState();
    j.setSetup({ access });
    if (!j.host || j.status !== "live") return;
    useJam.setState({ access });
    if (access === "closed") for (const k of j.knocks) this.turnAway(k.id);
    if (access === "open") for (const k of j.knocks) this.admit(k.id);
    if (j.listed) void (access === "closed" ? this.unlist(j.code) : this.announce());
    this.sendSoon();
  }

  /** put the session in the public list, or take it off */
  setListed(listed: boolean): void {
    const j = useJam.getState();
    j.setSetup({ listed });
    if (!j.host || j.status !== "live") return;
    useJam.setState({ listed });
    void (listed ? this.announce() : this.unlist(j.code));
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
   * A new code (and so a new link, QR code and Discord pass). The people in the session
   * move along on their own; the old code, and whoever only knows it, lead nowhere any
   * more. Whoever was removed may come back with the new code: it is a fresh start.
   */
  async changeCode(): Promise<void> {
    const j = useJam.getState();
    if (!j.host || j.status !== "live") return;
    const code = newCode();
    await Promise.all(j.people.filter((p) => !p.host).map((p) => this.wire.send({ t: "moved", to: p.id, sealed: this.wire.seal(code, p.id) })));
    for (const k of j.knocks) this.refuse(k.id, "Организатор сменил код сессии");
    try {
      await this.wire.join(code);
    } catch (e) {
      toast(errorText(e), "error");
      return;
    }
    if (j.listed) void this.unlist(j.code);
    this.banned.clear();
    this.pass = ticket();
    useJam.setState({ code, knocks: [] });
    this.sentRev = -1;
    this.sendState();
    if (j.listed) void this.announce();
    toast(`Новый код сессии: ${code}`);
  }

  // ---- the public list ------------------------------------------------------------------

  private async lobby(): Promise<JamLink> {
    if (!this.lobbyLink) {
      this.lobbyLink = await this.newLink((msg) => this.lobbyReceive(msg));
      await this.lobbyLink.join(LOBBY);
    }
    return this.lobbyLink;
  }

  /** the lobby is closed when nobody here looks at it or announces in it */
  private lobbyIdle(): void {
    const j = useJam.getState();
    if (this.browsing || (j.host && j.listed)) return;
    this.lobbyLink?.close();
    this.lobbyLink = null;
  }

  private async announce(): Promise<void> {
    const j = useJam.getState();
    if (!j.host || j.status !== "live" || !j.listed || j.access === "closed") return;
    const song = currentTrack();
    const link = await this.lobby().catch(() => null);
    void link?.send({ t: "open", code: j.code, host: j.name, people: j.people.length, access: j.access, song: song ? `${song.artists[0]?.name ?? ""} — ${song.title}` : undefined });
  }

  private async unlist(code: string): Promise<void> {
    await this.lobbyLink?.send({ t: "gone", code });
    this.lobbyIdle();
  }

  /** show the public list (while the join screen is open) or stop */
  async browse(on: boolean): Promise<void> {
    this.browsing = on;
    clearInterval(this.lobbyTimer);
    if (!on) {
      useJam.setState({ lobby: [] });
      this.lobbyIdle();
      return;
    }
    try {
      await this.lobby();
    } catch {
      return; // no connection: the list stays empty
    }
    this.lobbyTimer = setInterval(() => {
      const now = Date.now();
      useJam.setState((s) => (s.lobby.some((l) => now - l.seen > LISTED_FOR_MS) ? { lobby: s.lobby.filter((l) => now - l.seen <= LISTED_FOR_MS) } : s));
    }, 5000);
  }

  private lobbyReceive(msg: JamMessage): void {
    if (msg.t === "gone") {
      useJam.setState((s) => ({ lobby: s.lobby.filter((l) => l.code !== msg.code) }));
      return;
    }
    if (msg.t !== "open" || !this.browsing) return;
    const code = normalizeCode(String(msg.code));
    if (!code || code === useJam.getState().code) return;
    const entry = {
      code,
      host: String(msg.host ?? "").slice(0, 24) || "Кто-то",
      people: Math.max(1, Math.min(JAM_MAX, Number(msg.people) || 1)),
      access: msg.access === "ask" ? ("ask" as const) : ("open" as const),
      song: typeof msg.song === "string" ? msg.song.slice(0, 120) : undefined,
      seen: Date.now(),
    };
    useJam.setState((s) => ({ lobby: [...s.lobby.filter((l) => l.code !== code), entry].sort((a, b) => b.people - a.people).slice(0, 50) }));
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
        // heard here at once; the host's next report then agrees
        this.seekHeldUntil = Date.now() + SEEK_HOLD_MS;
        player.unsteered(() => player.seek(a.ms));
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
      case "wait":
        if (msg.to === this.wire.id && j.status === "joining") {
          useJam.setState({ waiting: true });
          this.waitFor(KNOCK_MS, "Организатор так и не ответил");
        }
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
      case "note":
        if (fromHost && msg.by !== this.wire.id) toast(String(msg.text).slice(0, 160));
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
    useJam.setState({ people: msg.people, control: msg.control, access: msg.access ?? "open", code: msg.code });
    if (msg.queue) {
      this.offset = msg.queue.offset;
      this.queueRev = msg.queue.rev;
      if (msg.queue.by) useJam.setState((s) => ({ addedBy: { ...s.addedBy, ...msg.queue!.by } }));
      usePlayer.setState({ queue: msg.queue.tracks, index: msg.now.index - this.offset });
    }
    // the queue changed and its new part has not come (yet, or at all): ask for it
    if (msg.rev !== this.queueRev) {
      if (Date.now() - this.lastResync > RESYNC_MS) {
        this.lastResync = Date.now();
        void this.wire.send({ t: "resync" });
      }
      return;
    }
    // where the host is counts from when the message came, by this device's own clock: the
    // clocks of two computers can differ by seconds (the song started 5–7 s in)
    this.follow({ ...msg.now, at: Date.now() - LATENCY_MS });
  }

  private welcome(hostId: string): void {
    clearTimeout(this.joinTimer);
    this.hostId = hostId;
    const s = usePlayer.getState();
    this.before = { queue: s.queue, index: s.index, positionMs: s.positionMs };
    useJam.setState({ status: "live", host: false, waiting: false });
    player.steerWith(this.steerGuest, true);
    this.timers.push(
      setInterval(() => void this.wire.send({ t: "here" }), HERE_MS),
      setInterval(() => {
        if (Date.now() - this.lastFromHost > HOST_GONE_MS) this.finish("Связь с организатором пропала, сессия закрыта");
      }, 5000),
    );
    toast(`Вы слушаете вместе с ${nameOf(hostId)}`);
  }

  /** play what the host plays, where the host is; and nothing else */
  private follow(now: JamNow): void {
    const s = usePlayer.getState();
    const local = now.index - this.offset;
    const track = s.queue[local];
    // nothing of the host's that can play here: this player stays quiet rather than going
    // on with something of its own
    const quiet = () => {
      if (s.status === "playing") player.unsteered(() => player.toggle());
    };
    if (!track || track.id !== now.trackId) return quiet();
    if (track.source === "local") {
      if (this.warnedLocal !== track.id) toast(`У организатора играет файл с его компьютера: «${track.title}». Здесь его не послушать.`);
      this.warnedLocal = track.id;
      return quiet();
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
      // this copy ended a moment before the host's: wait for the next song instead of starting over
      if (player.audio.ended) return;
      if (now.playing !== (s.status === "playing")) player.toggle();
      // right after a seek here, the host's older position is not a reason to jump back
      if (Date.now() > this.seekHeldUntil && Math.abs(s.positionMs - target) > DRIFT_MS) player.seek(target);
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
