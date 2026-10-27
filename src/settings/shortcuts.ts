// Keyboard shortcuts you can change. Bindings are written the way Electron writes them
// ("Ctrl+Shift+L", "Space", "MediaPlayPause"), so the same text works for keys inside the
// window and for system-wide keys registered by the main process.

export type ShortcutScope = "app" | "global" | "overlay";

export type ShortcutAction =
  | "playPause"
  | "next"
  | "prev"
  | "seekForward"
  | "seekBackward"
  | "volumeUp"
  | "volumeDown"
  | "mute"
  | "like"
  | "shuffle"
  | "repeat"
  | "search"
  | "settings"
  | "nowPlaying"
  | "toggleOverlay"
  | "showWindow"
  | "hideOverlay";

export interface ShortcutDef {
  id: string;
  action: ShortcutAction;
  scope: ShortcutScope;
  title: string;
  hint: string;
  default: string | null;
}

export const SHORTCUTS: ShortcutDef[] = [
  { id: "app.playPause", action: "playPause", scope: "app", title: "Играть / пауза", hint: "Остановить или продолжить текущий трек", default: "Space" },
  { id: "app.next", action: "next", scope: "app", title: "Следующий трек", hint: "Перейти к следующему в очереди", default: "Ctrl+Right" },
  { id: "app.prev", action: "prev", scope: "app", title: "Предыдущий трек", hint: "В начале трека — к предыдущему, иначе с начала", default: "Ctrl+Left" },
  { id: "app.seekForward", action: "seekForward", scope: "app", title: "Вперёд на 5 секунд", hint: "Перемотать вперёд", default: "Right" },
  { id: "app.seekBackward", action: "seekBackward", scope: "app", title: "Назад на 5 секунд", hint: "Перемотать назад", default: "Left" },
  { id: "app.volumeUp", action: "volumeUp", scope: "app", title: "Громче", hint: "Прибавить громкость на 5%", default: "Up" },
  { id: "app.volumeDown", action: "volumeDown", scope: "app", title: "Тише", hint: "Убавить громкость на 5%", default: "Down" },
  { id: "app.mute", action: "mute", scope: "app", title: "Без звука", hint: "Выключить и включить звук", default: "M" },
  { id: "app.like", action: "like", scope: "app", title: "Нравится", hint: "Добавить текущий трек в понравившиеся или убрать", default: "L" },
  { id: "app.shuffle", action: "shuffle", scope: "app", title: "Перемешать", hint: "Включить или выключить перемешивание", default: "S" },
  { id: "app.repeat", action: "repeat", scope: "app", title: "Повтор", hint: "Выключен → вся очередь → один трек", default: "R" },
  { id: "app.search", action: "search", scope: "app", title: "Поиск", hint: "Открыть поиск", default: "Ctrl+F" },
  { id: "app.settings", action: "settings", scope: "app", title: "Настройки", hint: "Открыть настройки", default: "Ctrl+," },
  { id: "app.nowPlaying", action: "nowPlaying", scope: "app", title: "Сейчас играет", hint: "Открыть экран с текстом и очередью", default: "Ctrl+N" },
  { id: "app.overlay", action: "toggleOverlay", scope: "app", title: "Мини-плеер", hint: "Показать или спрятать мини-плеер поверх окон", default: "Ctrl+M" },
  { id: "global.playPause", action: "playPause", scope: "global", title: "Играть / пауза", hint: "Из любой программы", default: "Ctrl+Alt+Space" },
  { id: "global.next", action: "next", scope: "global", title: "Следующий трек", hint: "Из любой программы", default: "Ctrl+Alt+Right" },
  { id: "global.prev", action: "prev", scope: "global", title: "Предыдущий трек", hint: "Из любой программы", default: "Ctrl+Alt+Left" },
  { id: "global.like", action: "like", scope: "global", title: "Нравится", hint: "Лайкнуть то, что играет, не открывая окно", default: "Ctrl+Alt+L" },
  { id: "global.volumeUp", action: "volumeUp", scope: "global", title: "Громче", hint: "Громкость EvPlayer, не системы", default: null },
  { id: "global.volumeDown", action: "volumeDown", scope: "global", title: "Тише", hint: "Громкость EvPlayer, не системы", default: null },
  { id: "global.overlay", action: "toggleOverlay", scope: "global", title: "Мини-плеер", hint: "Показать или спрятать мини-плеер поверх окон", default: "Ctrl+Alt+M" },
  { id: "global.show", action: "showWindow", scope: "global", title: "Показать EvPlayer", hint: "Вывести окно на передний план", default: "Ctrl+Alt+E" },
  { id: "overlay.playPause", action: "playPause", scope: "overlay", title: "Играть / пауза", hint: "Когда мини-плеер в фокусе (кликните по нему)", default: "Space" },
  { id: "overlay.next", action: "next", scope: "overlay", title: "Следующий трек", hint: "Когда мини-плеер в фокусе", default: "Right" },
  { id: "overlay.prev", action: "prev", scope: "overlay", title: "Предыдущий трек", hint: "Когда мини-плеер в фокусе", default: "Left" },
  { id: "overlay.seekForward", action: "seekForward", scope: "overlay", title: "Вперёд на 5 секунд", hint: "Когда мини-плеер в фокусе", default: "Shift+Right" },
  { id: "overlay.seekBackward", action: "seekBackward", scope: "overlay", title: "Назад на 5 секунд", hint: "Когда мини-плеер в фокусе", default: "Shift+Left" },
  { id: "overlay.volumeUp", action: "volumeUp", scope: "overlay", title: "Громче", hint: "Когда мини-плеер в фокусе", default: "Up" },
  { id: "overlay.volumeDown", action: "volumeDown", scope: "overlay", title: "Тише", hint: "Когда мини-плеер в фокусе", default: "Down" },
  { id: "overlay.mute", action: "mute", scope: "overlay", title: "Без звука", hint: "Когда мини-плеер в фокусе", default: "M" },
  { id: "overlay.like", action: "like", scope: "overlay", title: "Нравится", hint: "Когда мини-плеер в фокусе", default: "L" },
  { id: "overlay.show", action: "showWindow", scope: "overlay", title: "Открыть EvPlayer", hint: "Развернуть основное окно", default: "Enter" },
  { id: "overlay.hide", action: "hideOverlay", scope: "overlay", title: "Спрятать мини-плеер", hint: "Убрать мини-плеер с экрана", default: "Escape" },
];

const CODE_NAMES: Record<string, string> = {
  Space: "Space",
  ArrowRight: "Right",
  ArrowLeft: "Left",
  ArrowUp: "Up",
  ArrowDown: "Down",
  Comma: ",",
  Period: ".",
  Slash: "/",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  BracketLeft: "[",
  BracketRight: "]",
  Minus: "-",
  Equal: "=",
  Backquote: "`",
  Enter: "Enter",
  Tab: "Tab",
  Backspace: "Backspace",
  Delete: "Delete",
  Insert: "Insert",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
  Escape: "Escape",
  MediaPlayPause: "MediaPlayPause",
  MediaTrackNext: "MediaNextTrack",
  MediaTrackPrevious: "MediaPreviousTrack",
  MediaStop: "MediaStop",
};

/** the key itself, independent of the keyboard layout (a Russian layout still gives "L") */
export function keyName(code: string): string | null {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit\d$/.test(code)) return code.slice(5);
  if (/^Numpad\d$/.test(code)) return `num${code.slice(6)}`;
  if (/^F\d{1,2}$/.test(code)) return code;
  return CODE_NAMES[code] ?? null;
}

export function acceleratorFromEvent(e: Pick<KeyboardEvent, "code" | "ctrlKey" | "metaKey" | "altKey" | "shiftKey">): string | null {
  if (/^(Control|Shift|Alt|Meta|OS)(Left|Right)?$/.test(e.code)) return null;
  const key = keyName(e.code);
  if (!key) return null;
  const parts: string[] = [];
  if (e.ctrlKey || e.metaKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  parts.push(key);
  return parts.join("+");
}

const PRETTY: Record<string, string> = { Space: "Пробел", Right: "→", Left: "←", Up: "↑", Down: "↓", MediaPlayPause: "⏯", MediaNextTrack: "⏭", MediaPreviousTrack: "⏮", Escape: "Esc" };

export function prettyAccelerator(acc: string | null): string[] {
  if (!acc) return [];
  return acc.split("+").map((k) => PRETTY[k] ?? k);
}

export type Bindings = Record<string, string | null>;

export function binding(overrides: Bindings, def: ShortcutDef): string | null {
  return def.id in overrides ? overrides[def.id] : def.default;
}

/** another action of the same scope that already uses this key */
export function conflictFor(overrides: Bindings, id: string, acc: string): ShortcutDef | undefined {
  const me = SHORTCUTS.find((s) => s.id === id);
  return SHORTCUTS.find((s) => s.id !== id && s.scope === me?.scope && binding(overrides, s) === acc);
}

/** what the main process should register: accelerator -> action */
export function globalMap(overrides: Bindings): Record<string, ShortcutAction> {
  const out: Record<string, ShortcutAction> = {};
  for (const def of SHORTCUTS) {
    if (def.scope !== "global") continue;
    const acc = binding(overrides, def);
    if (acc) out[acc] = def.action;
  }
  return out;
}

export function actionFor(overrides: Bindings, scope: ShortcutScope, acc: string): ShortcutAction | null {
  for (const def of SHORTCUTS) if (def.scope === scope && binding(overrides, def) === acc) return def.action;
  return null;
}

export const appAction = (overrides: Bindings, acc: string) => actionFor(overrides, "app", acc);
