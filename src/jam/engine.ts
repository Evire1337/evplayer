import { finalizeEvent, generateSecretKey, getPublicKey, Relay } from "nostr-tools";
import { player } from "@/player/engine";
import { currentTrack, nextIndex, prevIndex, usePlayer, type PlayerStatus } from "@/player/store";
import type { Track } from "@/types";
import { toast } from "@/ui/toasts";
import {
  buildSyncPayload,
  calculateTargetPosition,
  generateRoomCode,
  normalizeRoomCode,
  shouldResyncPosition,
} from "./protocol";
import { useJam } from "./store";
import {
  DEFAULT_JAM_RELAYS,
  type JamControlMode,
  type JamMessage,
  type JamMessageContent,
  type JamParticipant,
  type JamSyncPayload,
} from "./types";

class JamEngine {
  private relays: Relay[] = [];
  private subscriptions: Array<{ close: () => void }> = [];
  private secretKey: Uint8Array = generateSecretKey();
  private myId: string = "";
  private currentRoomTag: string = "";
  private seenEvents = new Set<string>();
  private heartbeatInterval: ReturnType<typeof setInterval> | null = null;
  private isRemoteAction = false;
  private joinTimeout: ReturnType<typeof setTimeout> | null = null;

  private origMethods: {
    toggle: () => void;
    next: () => void;
    prev: (force?: boolean) => void;
    seek: (ms: number) => void;
    playIndex: (index: number) => Promise<void>;
    playQueue: (tracks: Track[], startIndex?: number) => Promise<void>;
    addToQueue: (track: Track) => void;
    playNext: (track: Track) => void;
    removeFromQueue: (index: number) => void;
    moveInQueue: (from: number, to: number) => void;
    clearUpcoming: () => void;
  };

  constructor() {
    this.myId = getPublicKey(this.secretKey);
    this.origMethods = {
      toggle: player.toggle.bind(player),
      next: player.next.bind(player),
      prev: player.prev.bind(player),
      seek: player.seek.bind(player),
      playIndex: player.playIndex.bind(player),
      playQueue: player.playQueue.bind(player),
      addToQueue: player.addToQueue.bind(player),
      playNext: player.playNext.bind(player),
      removeFromQueue: player.removeFromQueue.bind(player),
      moveInQueue: player.moveInQueue.bind(player),
      clearUpcoming: player.clearUpcoming.bind(player),
    };

    this.interceptPlayer();
    this.listenToPlayerChanges();
  }

  // Intercept player controls to route them through Jam when active
  private interceptPlayer(): void {
    player.toggle = () => {
      const jam = useJam.getState();
      if (!jam.active) return this.origMethods.toggle();
      if (jam.isHost) return this.origMethods.toggle();

      if (jam.controlMode === "host_only") {
        toast("Только организатор может управлять воспроизведением");
        return;
      }
      const state = usePlayer.getState();
      const newStatus = state.status === "playing" ? "paused" : "playing";
      void this.publish({
        type: "playback_state",
        status: newStatus,
        positionMs: state.positionMs,
        timestamp: Date.now(),
      });
    };

    player.next = () => {
      const jam = useJam.getState();
      if (!jam.active) return this.origMethods.next();
      if (jam.isHost) return this.origMethods.next();

      if (jam.controlMode === "host_only") {
        toast("Только организатор может переключать треки");
        return;
      }
      const state = usePlayer.getState();
      const idx = nextIndex(state, true);
      if (idx >= 0 && state.queue[idx]) {
        void this.publish({
          type: "track_change",
          track: state.queue[idx],
          index: idx,
          positionMs: 0,
          status: "playing",
          timestamp: Date.now(),
        });
      }
    };

    player.prev = (force?: boolean) => {
      const jam = useJam.getState();
      if (!jam.active) return this.origMethods.prev(force);
      if (jam.isHost) return this.origMethods.prev(force);

      if (jam.controlMode === "host_only") {
        toast("Только организатор может переключать треки");
        return;
      }
      const state = usePlayer.getState();
      if (!force && state.positionMs > 4000) {
        void this.publish({ type: "seek", positionMs: 0, timestamp: Date.now() });
        return;
      }
      const idx = prevIndex(state);
      if (idx >= 0 && state.queue[idx]) {
        void this.publish({
          type: "track_change",
          track: state.queue[idx],
          index: idx,
          positionMs: 0,
          status: "playing",
          timestamp: Date.now(),
        });
      }
    };

    player.seek = (ms: number) => {
      const jam = useJam.getState();
      if (!jam.active) return this.origMethods.seek(ms);
      if (jam.isHost) return this.origMethods.seek(ms);

      if (jam.controlMode === "host_only") {
        toast("Только организатор может перематывать");
        return;
      }
      void this.publish({ type: "seek", positionMs: ms, timestamp: Date.now() });
    };

    player.playIndex = async (index: number) => {
      const jam = useJam.getState();
      if (!jam.active) return this.origMethods.playIndex(index);
      if (jam.isHost) return this.origMethods.playIndex(index);

      if (jam.controlMode === "host_only") {
        toast("Только организатор может переключать треки");
        return;
      }
      const state = usePlayer.getState();
      const track = state.queue[index];
      if (track) {
        void this.publish({
          type: "track_change",
          track,
          index,
          positionMs: 0,
          status: "playing",
          timestamp: Date.now(),
        });
      }
    };

    player.addToQueue = (track: Track) => {
      const jam = useJam.getState();
      if (!jam.active) return this.origMethods.addToQueue(track);
      if (jam.isHost) {
        useJam.getState().recordTrackAddedBy(track.id, jam.myProfile.name);
        this.origMethods.addToQueue(track);
        return;
      }
      // Listener adding to shared Jam queue
      void this.publish({
        type: "queue_add",
        track,
        addedBy: jam.myProfile.name,
        playNext: false,
      });
      toast(`Добавлено в очередь Jam: ${track.title}`);
    };

    player.playNext = (track: Track) => {
      const jam = useJam.getState();
      if (!jam.active) return this.origMethods.playNext(track);
      if (jam.isHost) {
        useJam.getState().recordTrackAddedBy(track.id, jam.myProfile.name);
        this.origMethods.playNext(track);
        return;
      }
      void this.publish({
        type: "queue_add",
        track,
        addedBy: jam.myProfile.name,
        playNext: true,
      });
      toast(`Будет играть следующим в Jam: ${track.title}`);
    };

    player.removeFromQueue = (index: number) => {
      const jam = useJam.getState();
      if (!jam.active) return this.origMethods.removeFromQueue(index);
      if (jam.isHost) return this.origMethods.removeFromQueue(index);

      if (jam.controlMode === "host_only") {
        toast("Только организатор может удалять из очереди");
        return;
      }
      void this.publish({ type: "queue_remove", index });
    };

    player.moveInQueue = (from: number, to: number) => {
      const jam = useJam.getState();
      if (!jam.active) return this.origMethods.moveInQueue(from, to);
      if (jam.isHost) return this.origMethods.moveInQueue(from, to);

      if (jam.controlMode === "host_only") {
        toast("Только организатор может менять порядок в очереди");
        return;
      }
      void this.publish({ type: "queue_move", from, to });
    };

    player.clearUpcoming = () => {
      const jam = useJam.getState();
      if (!jam.active) return this.origMethods.clearUpcoming();
      if (jam.isHost) return this.origMethods.clearUpcoming();

      if (jam.controlMode === "host_only") {
        toast("Только организатор может очищать очередь");
        return;
      }
      const s = usePlayer.getState();
      void this.publish({
        type: "queue_update",
        queue: s.queue.slice(0, s.index + 1),
        index: s.index,
      });
    };
  }

  // Monitor host player actions to broadcast to listeners
  private listenToPlayerChanges(): void {
    let prevTrackId = "";
    let prevStatus: PlayerStatus = "idle";
    let prevIndex = -1;
    let prevQueueLength = 0;
    let lastSeekPos = 0;

    usePlayer.subscribe((state) => {
      if (this.isRemoteAction) return;
      const jam = useJam.getState();
      if (!jam.active || !jam.isHost) return;

      const track = currentTrack(state);
      const trackChanged = track?.id !== prevTrackId || state.index !== prevIndex;
      const statusChanged = state.status !== prevStatus && state.status !== "loading";
      const queueChanged = state.queue.length !== prevQueueLength;

      if (trackChanged && track) {
        prevTrackId = track.id;
        prevIndex = state.index;
        void this.publish({
          type: "track_change",
          track,
          index: state.index,
          positionMs: state.positionMs,
          status: state.status,
          timestamp: Date.now(),
        });
      }

      if (statusChanged) {
        prevStatus = state.status;
        void this.publish({
          type: "playback_state",
          status: state.status,
          positionMs: state.positionMs,
          timestamp: Date.now(),
        });
      }

      if (queueChanged) {
        prevQueueLength = state.queue.length;
        void this.publish({
          type: "queue_update",
          queue: state.queue,
          index: state.index,
        });
      }

      // Check for manual seek (jump of > 2500ms)
      const seekJump = Math.abs(state.positionMs - lastSeekPos);
      if (seekJump > 2500 && !trackChanged && state.status === "playing") {
        void this.publish({
          type: "seek",
          positionMs: state.positionMs,
          timestamp: Date.now(),
        });
      }
      lastSeekPos = state.positionMs;
    });
  }

  // --- CONNECTING TO RELAYS ---

  private async ensureRelaysConnected(): Promise<Relay[]> {
    if (this.relays.some((r) => r.connected)) {
      return this.relays.filter((r) => r.connected);
    }

    const connected: Relay[] = [];
    await Promise.all(
      DEFAULT_JAM_RELAYS.map(async (url) => {
        try {
          const r = await Relay.connect(url);
          connected.push(r);
        } catch {
          // fallback relay if one is down
        }
      }),
    );

    if (connected.length === 0) {
      throw new Error("Не удалось подключиться к сети серверов");
    }

    this.relays = connected;
    return connected;
  }

  private async publish(content: JamMessageContent): Promise<void> {
    if (!this.currentRoomTag) return;
    try {
      const activeRelays = await this.ensureRelaysConnected();
      const msg: JamMessage = { ...content, senderId: this.myId };
      const event = finalizeEvent(
        {
          kind: 20001,
          created_at: Math.floor(Date.now() / 1000),
          tags: [["t", this.currentRoomTag]],
          content: JSON.stringify(msg),
        },
        this.secretKey,
      );

      this.seenEvents.add(event.id);
      await Promise.any(activeRelays.map((r) => r.publish(event)));
    } catch {
      // ignore transient send failures
    }
  }

  private subscribeToRoom(roomTag: string): void {
    this.currentRoomTag = roomTag;
    for (const relay of this.relays) {
      if (!relay.connected) continue;
      const sub = relay.subscribe([{ kinds: [20001], "#t": [roomTag] }], {
        onevent: (event) => {
          if (this.seenEvents.has(event.id)) return;
          this.seenEvents.add(event.id);
          if (this.seenEvents.size > 800) {
            const first = this.seenEvents.values().next().value;
            if (first) this.seenEvents.delete(first);
          }

          try {
            const msg = JSON.parse(event.content) as JamMessage;
            if (msg.senderId === this.myId) return; // skip own messages
            this.handleIncomingMessage(msg);
          } catch {
            // ignore malformed message
          }
        },
      });
      this.subscriptions.push(sub);
    }
  }

  // --- HOST SESSION ---

  async startHost(controlMode: JamControlMode = "anyone"): Promise<string> {
    this.cleanup();
    const code = generateRoomCode();
    const roomTag = `evp_${code.toLowerCase().replace(/[^a-z0-9]/g, "")}`;
    const myProfile = useJam.getState().myProfile;

    useJam.getState().setStatus("hosting");

    try {
      await this.ensureRelaysConnected();
      this.subscribeToRoom(roomTag);

      const hostParticipant: JamParticipant = {
        id: this.myId,
        name: myProfile.name,
        avatarColor: myProfile.avatarColor,
        isHost: true,
        joinedAt: Date.now(),
      };

      useJam.getState().setSession({
        active: true,
        isHost: true,
        roomCode: code,
        hostName: myProfile.name,
        controlMode,
        participants: [hostParticipant],
      });

      this.startHeartbeat();
      toast(`Сессия Jam создана! Код: ${code}`);
      return code;
    } catch (err) {
      useJam.getState().setStatus("error", String(err));
      toast(`Ошибка создания сессии: ${err instanceof Error ? err.message : String(err)}`);
      throw err;
    }
  }

  // --- JOIN SESSION ---

  async joinSession(rawCode: string): Promise<void> {
    this.cleanup();
    const code = normalizeRoomCode(rawCode);
    const roomTag = `evp_${code.toLowerCase().replace(/[^a-z0-9]/g, "")}`;
    const myProfile = useJam.getState().myProfile;

    useJam.getState().setStatus("joining");

    return new Promise((resolve, reject) => {
      this.ensureRelaysConnected()
        .then(() => {
          this.subscribeToRoom(roomTag);

          // Listen for host response
          const onResponse = () => {
            if (this.joinTimeout) {
              clearTimeout(this.joinTimeout);
              this.joinTimeout = null;
            }
            resolve();
          };

          // Send join request
          void this.publish({
            type: "join_request",
            name: myProfile.name,
            avatarColor: myProfile.avatarColor,
          });

          // Timeout if host not found in 8 seconds
          this.joinTimeout = setTimeout(() => {
            if (!useJam.getState().active) {
              useJam.getState().setStatus("error", "Сессия не найдена");
              toast("Организатор не найден. Проверьте код сессии.");
              this.cleanup();
              reject(new Error("Host not found"));
            }
          }, 8000);

          (this as unknown as { _onJoinSuccess?: () => void })._onJoinSuccess = onResponse;
        })
        .catch((err) => {
          useJam.getState().setStatus("error", "Ошибка соединения");
          toast(`Ошибка подключения к серверам: ${err.message}`);
          reject(err);
        });
    });
  }

  // --- MESSAGE HANDLER ---

  private handleIncomingMessage(msg: JamMessage): void {
    const jam = useJam.getState();

    if (jam.isHost) {
      this.handleHostMessage(msg);
    } else {
      this.handleClientMessage(msg);
    }
  }

  private handleHostMessage(msg: JamMessage): void {
    const jam = useJam.getState();

    switch (msg.type) {
      case "join_request": {
        const participant: JamParticipant = {
          id: msg.senderId,
          name: msg.name || "Слушатель",
          avatarColor: msg.avatarColor || "#1db954",
          isHost: false,
          joinedAt: Date.now(),
        };

        const updated = [...jam.participants.filter((p) => p.id !== msg.senderId), participant];
        jam.updateParticipants(updated);

        const syncPayload = this.buildCurrentSyncPayload(updated);
        void this.publish({
          type: "join_response",
          targetId: msg.senderId,
          ok: true,
          state: syncPayload,
        });

        void this.publish({ type: "sync", state: syncPayload });
        toast(`${participant.name} подключился к сессии!`);
        break;
      }

      case "playback_state": {
        if (jam.controlMode === "host_only") return;
        this.withRemoteLock(() => {
          if (msg.status === "playing" && usePlayer.getState().status !== "playing") {
            this.origMethods.toggle();
          } else if (msg.status === "paused" && usePlayer.getState().status === "playing") {
            this.origMethods.toggle();
          }
        });
        void this.publish(msg);
        break;
      }

      case "seek": {
        if (jam.controlMode === "host_only") return;
        this.withRemoteLock(() => {
          this.origMethods.seek(msg.positionMs);
        });
        void this.publish(msg);
        break;
      }

      case "track_change": {
        if (jam.controlMode === "host_only") return;
        this.withRemoteLock(() => {
          void this.origMethods.playIndex(msg.index);
        });
        void this.publish(msg);
        break;
      }

      case "queue_add": {
        jam.recordTrackAddedBy(msg.track.id, msg.addedBy);
        if (msg.playNext) {
          this.origMethods.playNext(msg.track);
        } else {
          this.origMethods.addToQueue(msg.track);
        }
        toast(`${msg.addedBy} добавил(а) трек: ${msg.track.title}`);
        void this.publish({
          type: "queue_update",
          queue: usePlayer.getState().queue,
          index: usePlayer.getState().index,
        });
        break;
      }

      case "queue_remove": {
        if (jam.controlMode === "host_only") return;
        this.origMethods.removeFromQueue(msg.index);
        void this.publish({
          type: "queue_update",
          queue: usePlayer.getState().queue,
          index: usePlayer.getState().index,
        });
        break;
      }

      case "queue_move": {
        if (jam.controlMode === "host_only") return;
        this.origMethods.moveInQueue(msg.from, msg.to);
        void this.publish({
          type: "queue_update",
          queue: usePlayer.getState().queue,
          index: usePlayer.getState().index,
        });
        break;
      }

      case "reaction": {
        jam.addReaction(msg);
        break;
      }

      case "chat": {
        jam.addChatMessage(msg);
        break;
      }

      case "leave": {
        const leaving = jam.participants.find((p) => p.id === msg.senderId);
        if (leaving) {
          const updated = jam.participants.filter((p) => p.id !== msg.senderId);
          jam.updateParticipants(updated);
          void this.publish({
            type: "sync",
            state: this.buildCurrentSyncPayload(updated),
          });
          toast(`${leaving.name} покинул(а) сессию`);
        }
        break;
      }
    }
  }

  private handleClientMessage(msg: JamMessage): void {
    const jam = useJam.getState();

    switch (msg.type) {
      case "join_response": {
        if (msg.targetId === this.myId && msg.ok && msg.state) {
          useJam.getState().setSession({
            active: true,
            isHost: false,
            roomCode: msg.state.roomCode,
            hostName: msg.state.hostName,
            controlMode: msg.state.controlMode,
            participants: msg.state.participants,
          });
          this.applySyncPayload(msg.state);
          toast(`Вы подключились к Jam (${msg.state.hostName})!`);
          const cb = (this as unknown as { _onJoinSuccess?: () => void })._onJoinSuccess;
          if (cb) cb();
        }
        break;
      }

      case "sync": {
        jam.updateParticipants(msg.state.participants);
        jam.setControlMode(msg.state.controlMode);
        this.applySyncPayload(msg.state);
        break;
      }

      case "playback_state": {
        this.withRemoteLock(() => {
          const currentStatus = usePlayer.getState().status;
          if (msg.status === "playing" && currentStatus !== "playing") {
            this.origMethods.toggle();
          } else if (msg.status === "paused" && currentStatus === "playing") {
            this.origMethods.toggle();
          }
          const target = calculateTargetPosition(msg.positionMs, msg.timestamp, Date.now(), msg.status);
          if (shouldResyncPosition(usePlayer.getState().positionMs, target)) {
            this.origMethods.seek(target);
          }
        });
        break;
      }

      case "seek": {
        this.withRemoteLock(() => {
          const target = calculateTargetPosition(msg.positionMs, msg.timestamp, Date.now(), "playing");
          this.origMethods.seek(target);
        });
        break;
      }

      case "track_change": {
        this.withRemoteLock(() => {
          const s = usePlayer.getState();
          if (s.queue[msg.index]?.id !== msg.track.id) {
            usePlayer.setState({ index: msg.index });
          }
          void this.origMethods.playIndex(msg.index).then(() => {
            const target = calculateTargetPosition(
              msg.positionMs,
              msg.timestamp,
              Date.now(),
              msg.status,
              msg.track.durationMs,
            );
            if (target > 1000) this.origMethods.seek(target);
          });
        });
        break;
      }

      case "queue_update": {
        this.withRemoteLock(() => {
          usePlayer.setState({ queue: msg.queue, index: msg.index });
        });
        break;
      }

      case "control_mode": {
        jam.setControlMode(msg.mode);
        toast(`Режим управления: ${msg.mode === "anyone" ? "Все участники" : "Только хост"}`);
        break;
      }

      case "reaction": {
        jam.addReaction(msg);
        break;
      }

      case "chat": {
        jam.addChatMessage(msg);
        break;
      }

      case "kick": {
        if (msg.participantId === this.myId) {
          toast("Организатор исключил вас из сессии");
          this.cleanup();
        }
        break;
      }

      case "session_ended": {
        toast("Организатор завершил совместную сессию");
        this.cleanup();
        break;
      }
    }
  }

  // --- PLAYBACK SYNC APPLICATION ---

  private applySyncPayload(payload: JamSyncPayload): void {
    if (!payload.track) return;

    if (payload.track.source === "local") {
      toast(`Хост играет локальный файл: «${payload.track.title}». На других устройствах он может быть недоступен.`);
      return;
    }

    this.withRemoteLock(() => {
      const state = usePlayer.getState();
      const current = currentTrack(state);

      // Track or index mismatch: load the correct track from queue
      if (current?.id !== payload.track?.id || state.index !== payload.index) {
        usePlayer.setState({ queue: payload.queue, index: payload.index });
        void this.origMethods.playIndex(payload.index).then(() => {
          const target = calculateTargetPosition(
            payload.positionMs,
            payload.timestamp,
            Date.now(),
            payload.status,
            payload.track?.durationMs,
          );
          if (target > 1000) this.origMethods.seek(target);
          if (payload.status === "paused") this.origMethods.toggle();
        });
        return;
      }

      // Same track: sync queue if length differs
      if (state.queue.length !== payload.queue.length) {
        usePlayer.setState({ queue: payload.queue });
      }

      // Check play / pause
      if (payload.status === "playing" && state.status !== "playing") {
        this.origMethods.toggle();
      } else if (payload.status === "paused" && state.status === "playing") {
        this.origMethods.toggle();
      }

      // Check drift
      const targetPos = calculateTargetPosition(
        payload.positionMs,
        payload.timestamp,
        Date.now(),
        payload.status,
        payload.track?.durationMs,
      );

      if (shouldResyncPosition(state.positionMs, targetPos, 1500)) {
        this.origMethods.seek(targetPos);
      }
    });
  }

  // --- ACTIONS (REACTIONS, CHAT, CONTROLS) ---

  sendReaction(emoji: string): void {
    const jam = useJam.getState();
    if (!jam.active) return;

    const reaction = {
      type: "reaction" as const,
      id: crypto.randomUUID(),
      emoji,
      fromName: jam.myProfile.name,
      timestamp: Date.now(),
    };

    jam.addReaction(reaction);
    void this.publish(reaction);
  }

  sendChat(text: string): void {
    const jam = useJam.getState();
    if (!jam.active || !text.trim()) return;

    const msg = {
      type: "chat" as const,
      id: crypto.randomUUID(),
      text: text.trim().slice(0, 200),
      fromName: jam.myProfile.name,
      timestamp: Date.now(),
    };

    jam.addChatMessage(msg);
    void this.publish(msg);
  }

  setControlMode(mode: JamControlMode): void {
    const jam = useJam.getState();
    if (!jam.active || !jam.isHost) return;

    jam.setControlMode(mode);
    void this.publish({ type: "control_mode", mode });
  }

  kickParticipant(participantId: string): void {
    const jam = useJam.getState();
    if (!jam.active || !jam.isHost) return;

    void this.publish({ type: "kick", participantId });
    const updated = jam.participants.filter((p) => p.id !== participantId);
    jam.updateParticipants(updated);
    void this.publish({
      type: "sync",
      state: this.buildCurrentSyncPayload(updated),
    });
  }

  leaveOrEndSession(): void {
    const jam = useJam.getState();
    if (!jam.active) return;

    if (jam.isHost) {
      void this.publish({ type: "session_ended", reason: "Host ended session" });
      toast("Сессия Jam завершена");
    } else {
      void this.publish({ type: "leave", participantId: this.myId });
      toast("Вы покинули сессию Jam");
    }

    this.cleanup();
  }

  // --- UTILS & HEARTBEAT ---

  private startHeartbeat(): void {
    if (this.heartbeatInterval) clearInterval(this.heartbeatInterval);
    this.heartbeatInterval = setInterval(() => {
      const jam = useJam.getState();
      if (!jam.active || !jam.isHost) return;
      void this.publish({
        type: "sync",
        state: this.buildCurrentSyncPayload(jam.participants),
      });
    }, 3000);
  }

  private buildCurrentSyncPayload(participants: JamParticipant[]): JamSyncPayload {
    const state = usePlayer.getState();
    const track = currentTrack(state) ?? null;
    const jam = useJam.getState();

    return buildSyncPayload(
      state,
      jam.controlMode,
      participants,
      jam.hostName || jam.myProfile.name,
      jam.roomCode,
      track,
    );
  }

  private withRemoteLock(fn: () => void): void {
    this.isRemoteAction = true;
    try {
      fn();
    } finally {
      setTimeout(() => {
        this.isRemoteAction = false;
      }, 300);
    }
  }

  private cleanup(): void {
    if (this.heartbeatInterval) {
      clearInterval(this.heartbeatInterval);
      this.heartbeatInterval = null;
    }

    if (this.joinTimeout) {
      clearTimeout(this.joinTimeout);
      this.joinTimeout = null;
    }

    for (const sub of this.subscriptions) {
      try {
        sub.close();
      } catch {
        // ignore
      }
    }
    this.subscriptions = [];

    for (const relay of this.relays) {
      try {
        relay.close();
      } catch {
        // ignore
      }
    }
    this.relays = [];
    this.currentRoomTag = "";
    this.seenEvents.clear();

    useJam.getState().resetSession();
  }
}

export const jamEngine = new JamEngine();
