import { useJam } from "./store";
import { useEffect, useState } from "react";

export default function ReactionOverlay() {
  const reactions = useJam((s) => s.reactions);
  const chatMessages = useJam((s) => s.chatMessages);
  const [, setTick] = useState(0);

  // Periodically re-render to clean up expired animations
  useEffect(() => {
    if (reactions.length === 0 && chatMessages.length === 0) return;
    const interval = setInterval(() => setTick((t) => t + 1), 1000);
    return () => clearInterval(interval);
  }, [reactions.length, chatMessages.length]);

  const now = Date.now();
  const visibleReactions = reactions.filter((r) => now - (r.receivedAt || r.timestamp) < 4500);
  const visibleChat = chatMessages.filter((m) => now - (m.receivedAt || m.timestamp) < 6000);

  if (visibleReactions.length === 0 && visibleChat.length === 0) return null;

  return (
    <div className="jam-overlay-container" aria-live="polite">
      {/* Floating emoji reactions */}
      <div className="jam-reactions-stream">
        {visibleReactions.map((r, index) => {
          const age = now - (r.receivedAt || r.timestamp);
          const leftOffset = (index * 26) % 110;
          return (
            <div
              key={r.id}
              className="jam-floating-reaction"
              style={{
                left: `${leftOffset}px`,
                animationDuration: "4s",
                opacity: Math.max(0, 1 - age / 4000),
              }}
            >
              <span className="jam-reaction-emoji">{r.emoji}</span>
              <span className="jam-reaction-author">{r.fromName}</span>
            </div>
          );
        })}
      </div>

      {/* Floating chat messages */}
      <div className="jam-chat-stream">
        {visibleChat.map((m) => {
          const age = now - (m.receivedAt || m.timestamp);
          return (
            <div
              key={m.id}
              className="jam-chat-bubble"
              style={{
                opacity: Math.max(0, 1 - age / 5500),
              }}
            >
              <strong className="jam-chat-author">{m.fromName}:</strong> {m.text}
            </div>
          );
        })}
      </div>
    </div>
  );
}
