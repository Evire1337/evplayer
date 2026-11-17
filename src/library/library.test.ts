import { beforeEach, describe, expect, it } from "vitest";
import type { Track } from "@/types";
import { moveItem, useLibrary } from "./store";
import { decodePlaylistCode, encodePlaylistCode, extractPlaylistCodes, looksLikePlaylistCode } from "./playlistCode";

const t = (id: string, source: Track["source"] = "soundcloud"): Track => ({ id, source, title: `Трек ${id}`, artists: [{ id: "a", name: "Артист" }], durationMs: 1000 });

describe("library", () => {
  beforeEach(() => useLibrary.setState({ likes: [], playlists: [] }));

  it("likes and unlikes, newest first, and reorders", () => {
    const lib = useLibrary.getState();
    lib.toggleLike(t("1"));
    lib.toggleLike(t("2"));
    expect(useLibrary.getState().likes.map((l) => l.track.id)).toEqual(["2", "1"]);
    useLibrary.getState().moveLike("1", 0);
    expect(useLibrary.getState().likes.map((l) => l.track.id)).toEqual(["1", "2"]);
    useLibrary.getState().toggleLike(t("1"));
    expect(useLibrary.getState().likes.map((l) => l.track.id)).toEqual(["2"]);
  });

  it("keeps mixed-source playlists without duplicates", () => {
    const p = useLibrary.getState().createPlaylist("  Микс  ");
    expect(p.title).toBe("Микс");
    expect(useLibrary.getState().addToPlaylist(p.id, [t("sc:1"), t("yt:2", "youtube")])).toBe(2);
    expect(useLibrary.getState().addToPlaylist(p.id, [t("sc:1")])).toBe(0);
    const items = useLibrary.getState().playlists[0].items;
    useLibrary.getState().moveInPlaylist(p.id, items[1].id, 0);
    expect(useLibrary.getState().playlists[0].items.map((i) => i.track.id)).toEqual(["yt:2", "sc:1"]);
  });

  it("moves items safely", () => {
    expect(moveItem([1, 2, 3], 0, 2)).toEqual([2, 3, 1]);
    expect(moveItem([1, 2, 3], 5, 0)).toEqual([1, 2, 3]);
  });

  it("round-trips a playlist code", async () => {
    const code = await encodePlaylistCode({ title: "Ночь", icon: "data:image/jpeg;base64,AAA", tracks: [t("sc:1"), t("yt:2", "youtube")] });
    expect(looksLikePlaylistCode(code)).toBe(true);
    const back = await decodePlaylistCode(code);
    expect(back.title).toBe("Ночь");
    expect(back.tracks.map((x) => [x.id, x.source])).toEqual([["sc:1", "soundcloud"], ["yt:2", "youtube"]]);
  });
});


describe("playlist codes pasted from a text file", () => {
  const list = (title: string) => ({ title, tracks: [{ id: "sc:1", source: "soundcloud" as const, title: "Song", durationMs: 1000, artists: [{ id: "a", name: "A" }] }] });

  it("finds every code in a file, one per line, with names and empty lines between", async () => {
    const one = await encodePlaylistCode(list("One"));
    const two = await encodePlaylistCode(list("Two"));
    const file = `﻿Мои плейлисты\r\n\r\nПервый:\r\n${one}\r\n\r\nВторой\r\n${two}\r\n`;
    expect(extractPlaylistCodes(file)).toEqual([one, two]);
    expect((await decodePlaylistCode(extractPlaylistCodes(file)[1])).title).toBe("Two");
  });

  it("joins one code that was wrapped over several lines", async () => {
    const code = await encodePlaylistCode(list("Wrapped"));
    const wrapped = code.replace(/(.{40})/g, "$1\n");
    expect(extractPlaylistCodes(wrapped)).toEqual([code]);
    expect((await decodePlaylistCode(wrapped)).title).toBe("Wrapped");
  });

  it("ignores invisible characters and padding", async () => {
    const code = await encodePlaylistCode(list("Quiet"));
    expect((await decodePlaylistCode(`​${code}==​`)).title).toBe("Quiet");
  });

  it("does not take plain text for a code", () => {
    expect(looksLikePlaylistCode("Исполнитель — Название")).toBe(false);
    expect(extractPlaylistCodes("hello")).toEqual([]);
  });
});
