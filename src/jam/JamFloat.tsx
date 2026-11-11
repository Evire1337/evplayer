import { useEffect, useState } from "react";
import { useJam } from "./store";
import Emoji from "./Emoji";

const REACTION_MS = 4000;
const MESSAGE_MS = 6000;

/**
 * Reactions and chat messages float up in the corner for a few seconds (the chat only
 * while the dialog is closed: in it, the messages are right there). The fading is CSS; this
 * only takes them away when it is over, with one timer for the next one due.
 */
export default function JamFloat() {
  const reactions = useJam((s) => s.reactions);
  const chat = useJam((s) => s.chat);
  const dialogOpen = useJam((s) => s.open);
  const [now, setNow] = useState(Date.now);

  const shownReactions = reactions.filter((r) => now - r.at < REACTION_MS);
  const shownChat = dialogOpen ? [] : chat.filter((m) => now - m.at < MESSAGE_MS).slice(-3);

  useEffect(() => {
    const ends = [...shownReactions.map((r) => r.at + REACTION_MS), ...shownChat.map((m) => m.at + MESSAGE_MS)];
    if (!ends.length) return;
    const timer = setTimeout(() => setNow(Date.now()), Math.max(50, Math.min(...ends) - Date.now()));
    return () => clearTimeout(timer);
  });

  // something new came in: look at the clock again so it shows
  useEffect(() => setNow(Date.now()), [reactions.length, chat.length]);

  if (!shownReactions.length && !shownChat.length) return null;
  return (
    <div className="jam-float" aria-live="polite">
      <div className="jam-float-reactions">
        {shownReactions.map((r, i) => (
          <div key={r.id} className="jam-float-reaction" style={{ left: (i * 26) % 100 }}>
            <span className="jam-float-emoji"><Emoji emoji={r.emoji} size={44} /></span>
            <span className="jam-float-from">{r.from}</span>
          </div>
        ))}
      </div>
      <div className="jam-float-chat">
        {shownChat.map((m) => (
          <div key={m.id} className="jam-float-message"><b>{m.from}:</b> {m.text}</div>
        ))}
      </div>
    </div>
  );
}
