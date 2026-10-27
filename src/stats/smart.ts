import type { SmartPlaylistKind } from "./listeningStats";

export const SMART_INFO: Record<SmartPlaylistKind, { title: string; description: string; hue: number }> = {
  on_repeat: { title: "На повторе", description: "То, что вы слушали 3 и больше раз за последний месяц", hue: 160 },
  discoveries: { title: "Новые открытия", description: "Песни, которые вы впервые услышали за последние две недели", hue: 200 },
  forgotten: { title: "Забытые любимые", description: "Часто слушали раньше, но не включали полтора месяца", hue: 28 },
  liked_rare: { title: "Лайкнули и забыли", description: "Понравившиеся, которые вы почти не слушали", hue: 330 },
};
