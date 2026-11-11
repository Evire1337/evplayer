import { useState } from "react";

// Reactions drawn the same everywhere, in a round 3D style close to the iPhone's (Apple's own
// emoji may not be used outside Apple's systems): Microsoft's Fluent Emoji, MIT licence.
// Loaded from a CDN the first time they show; the plain character stays if one cannot load.

const BASE = "https://cdn.jsdelivr.net/gh/microsoft/fluentui-emoji@main/assets/";

const FILES: Record<string, string> = {
  "🔥": "Fire/3D/fire_3d.png",
  "❤️": "Red heart/3D/red_heart_3d.png",
  "👏": "Clapping hands/Default/3D/clapping_hands_3d_default.png",
  "🎉": "Party popper/3D/party_popper_3d.png",
  "🚀": "Rocket/3D/rocket_3d.png",
  "😭": "Loudly crying face/3D/loudly_crying_face_3d.png",
  "⚡": "High voltage/3D/high_voltage_3d.png",
  "🎧": "Headphone/3D/headphone_3d.png",
};

export const emojiUrl = (emoji: string) => (FILES[emoji] ? BASE + FILES[emoji].split("/").map(encodeURIComponent).join("/") : null);

export default function Emoji({ emoji, size }: { emoji: string; size: number }) {
  const [broken, setBroken] = useState(false);
  const url = emojiUrl(emoji);
  if (!url || broken) return <span style={{ fontSize: size * 0.85, lineHeight: 1 }}>{emoji}</span>;
  return <img className="jam-emoji" src={url} alt={emoji} width={size} height={size} draggable={false} onError={() => setBroken(true)} />;
}
