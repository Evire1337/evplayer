import { beforeEach, describe, expect, it } from "vitest";
import { useJam } from "./store";
import "@/jam/engine";
import type { JamParticipant, JamReaction } from "./types";
import { player } from "@/player/engine";
import { usePlayer } from "@/player/store";
import type { Track } from "@/types";

describe("jam store and flow", () => {
  beforeEach(() => {
    window.HTMLMediaElement.prototype.play = () => Promise.resolve();
    window.HTMLMediaElement.prototype.pause = () => {};
    window.HTMLMediaElement.prototype.load = () => {};
    useJam.getState().resetSession();
    usePlayer.setState({ queue: [], index: -1, status: "idle", positionMs: 0 });
  });

  it("initializes with default profile and inactive session", () => {
    const s = useJam.getState();
    expect(s.active).toBe(false);
    expect(s.isHost).toBe(false);
    expect(s.roomCode).toBe("");
    expect(s.participants).toEqual([]);
    expect(s.myProfile.name).toBeTruthy();
    expect(s.myProfile.avatarColor).toMatch(/^#[0-9a-f]{6}$/i);
  });

  it("updates user profile", () => {
    useJam.getState().setProfile({ name: "DJ Evire", avatarColor: "#ff5500" });
    const s = useJam.getState();
    expect(s.myProfile.name).toBe("DJ Evire");
    expect(s.myProfile.avatarColor).toBe("#ff5500");
  });

  it("manages session lifecycle", () => {
    const host: JamParticipant = {
      id: "peer-host",
      name: "HostUser",
      avatarColor: "#1db954",
      isHost: true,
      joinedAt: 1000,
    };
    useJam.getState().setSession({
      active: true,
      isHost: true,
      roomCode: "JAM-1234",
      hostName: "HostUser",
      controlMode: "anyone",
      participants: [host],
    });

    let state = useJam.getState();
    expect(state.active).toBe(true);
    expect(state.isHost).toBe(true);
    expect(state.roomCode).toBe("JAM-1234");
    expect(state.participants.length).toBe(1);

    // Participant joins
    const listener: JamParticipant = {
      id: "peer-listener",
      name: "Listener1",
      avatarColor: "#3b82f6",
      isHost: false,
      joinedAt: 2000,
    };
    useJam.getState().updateParticipants([...state.participants, listener]);
    expect(useJam.getState().participants.length).toBe(2);

    // Session resets
    useJam.getState().resetSession();
    state = useJam.getState();
    expect(state.active).toBe(false);
    expect(state.participants.length).toBe(0);
    expect(state.roomCode).toBe("");
  });

  it("adds and manages live reactions", () => {
    const reaction: JamReaction = {
      id: "r1",
      emoji: "🔥",
      fromName: "Alice",
      timestamp: Date.now(),
    };
    useJam.getState().addReaction(reaction);
    const r = useJam.getState().reactions[0];
    expect(r.emoji).toBe("🔥");
    expect(r.fromName).toBe("Alice");
    expect(r.receivedAt).toBeGreaterThan(0);
  });

  it("records track attribution in shared queue", () => {
    useJam.getState().recordTrackAddedBy("sc:track-99", "Bob");
    expect(useJam.getState().trackAddedBy["sc:track-99"]).toBe("Bob");
  });

  it("handles control mode toggling", () => {
    useJam.getState().setControlMode("host_only");
    expect(useJam.getState().controlMode).toBe("host_only");
    useJam.getState().setControlMode("anyone");
    expect(useJam.getState().controlMode).toBe("anyone");
  });

  it("intercepts player queue addition when in Jam session", () => {
    const track: Track = {
      id: "yt:cool-song",
      source: "youtube",
      title: "Cool Song",
      artists: [{ id: "art1", name: "Artist" }],
      durationMs: 200_000,
    };

    useJam.getState().setSession({
      active: true,
      isHost: true,
      roomCode: "JAM-7777",
      hostName: "HostUser",
      controlMode: "anyone",
      participants: [],
    });

    player.addToQueue(track);
    expect(usePlayer.getState().queue).toContainEqual(track);
    expect(useJam.getState().trackAddedBy["yt:cool-song"]).toBeTruthy();
  });

  it("handles chat messages in jam session", () => {
    useJam.getState().addChatMessage({
      id: "chat-1",
      text: "Great track!",
      fromName: "Alex",
      timestamp: Date.now(),
    });
    expect(useJam.getState().chatMessages.length).toBe(1);
    expect(useJam.getState().chatMessages[0].text).toBe("Great track!");
  });
});
