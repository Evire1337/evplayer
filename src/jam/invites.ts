import type { DiscordUser } from "@/platform/api";
import { toast } from "@/ui/toasts";
import { playLink } from "@/app/playLink";
import { useNavigation } from "@/ui/navigation";
import { jam } from "./engine";
import { normalizeCode } from "./protocol";
import { useJam } from "./store";

// The ways in from outside: an evplayer://jam/... link (a QR code, a link sent in a chat),
// Discord's "Ask to join" on the status, and evplayer://play/... (Discord's "Слушать").

/** Discord needs a little time to show the status with the session's code before it hands
    that code to the friend */
const STATUS_SETTLES_MS = 1500;
const ASKS_KEPT = 5;

/** an evplayer:// link: a song to play (from Discord's "Слушать") or a session to join */
function openLink(url: string): void {
  void playLink(url).then((played) => {
    if (played) useNavigation.getState().go("now");
    else openInvite(url);
  });
}

/** a link or a code from outside: open the session dialog and go in */
export function openInvite(text: string): void {
  const code = normalizeCode(text);
  if (!code) return;
  const s = useJam.getState();
  s.setOpen(true);
  // the whole text: an invite from Discord carries the host's pass after the code
  if (s.status === "idle") void jam.join(text);
  else if (s.code !== code) toast("Чтобы войти в другую сессию, сначала выйдите из этой");
}

/** the name for the session when there is none yet: the one from Discord */
function nameFrom(user: DiscordUser | null | undefined): void {
  if (user && !useJam.getState().name.trim()) useJam.getState().setProfile({ name: user.globalName ?? user.username });
}

/** let a friend from Discord in: start a session if there is none, then send the invite */
export async function letIn(user: DiscordUser): Promise<void> {
  const api = window.evplayer;
  useJam.setState((s) => ({ asks: s.asks.filter((a) => a.id !== user.id) }));
  if (!api) return;
  if (useJam.getState().status === "idle") {
    nameFrom((await api.discord.status().catch(() => null))?.user);
    await jam.start();
    if (useJam.getState().status !== "live") return;
    await new Promise((r) => setTimeout(r, STATUS_SETTLES_MS));
  }
  api.discord.answer(user.id, true);
  toast(`${user.globalName ?? user.username} сейчас подключится`);
}

export function turnDown(user: DiscordUser): void {
  useJam.setState((s) => ({ asks: s.asks.filter((a) => a.id !== user.id) }));
  window.evplayer?.discord.answer(user.id, false);
}

/** call once when the app starts */
export function startInvites(): () => void {
  const api = window.evplayer;
  if (!api) return () => undefined;
  const offDiscord = api.discord.onEvent((event) => {
    if (event.kind === "request") {
      useJam.setState((s) => ({ asks: [...s.asks.filter((a) => a.id !== event.user.id), event.user].slice(-ASKS_KEPT) }));
    } else if (normalizeCode(event.secret)) {
      void api.discord.status().then((st) => nameFrom(st.user)).catch(() => undefined).finally(() => openInvite(event.secret));
    } else {
      toast("Друг ещё не начал сессию: попросите его прислать код", "error");
    }
  });
  void api.links.first().then((url) => url && openLink(url)).catch(() => undefined);
  const offLinks = api.links.onOpen(openLink);
  return () => {
    offDiscord();
    offLinks();
  };
}
