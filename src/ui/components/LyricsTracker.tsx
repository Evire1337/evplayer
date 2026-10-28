import { useEffect } from "react";
import { currentTrack, usePlayer } from "@/player/store";
import { useActiveLine } from "../lyricsTiming";
import { useLyrics } from "../useLyrics";
import { useLyricsLine } from "../lyricsLine";

// follows the current line for the mini player while it is open; renders nothing
export default function LyricsTracker() {
  const track = usePlayer((s) => currentTrack(s));
  const lyrics = useLyrics(track);
  const lines = lyrics.status === "ready" && lyrics.synced ? lyrics.lines : null;
  const index = useActiveLine(lines, track?.id);
  const line = lines && index >= 0 ? lines[index].text || "♪" : null;
  useEffect(() => {
    useLyricsLine.setState({ line });
  }, [line]);
  useEffect(() => () => useLyricsLine.setState({ line: null }), []);
  return null;
}
