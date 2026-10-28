import { spotifyClientId, useSettings } from "@/settings/store";
import { useLibrary } from "@/library/store";
import { matchTracks } from "@/library/matchTracks";
import { useImportProgress } from "@/library/importProgress";
import { toast } from "@/ui/toasts";
import { spotifyCollection, spotifyLikedSongs, spotifyMe, spotifyMyPlaylists } from "./webApi";

// Your Spotify account in EvPlayer: sign in once, then carry over "Liked Songs" and your
// playlists, any length. Songs are found on YouTube Music and SoundCloud.

export async function signInSpotify(): Promise<void> {
  const clientId = spotifyClientId();
  if (!clientId) throw new Error("Нужен Client ID приложения Spotify — см. «Своё приложение» ниже");
  const bridge = window.evplayer?.spotify;
  if (!bridge) throw new Error("Вход в Spotify работает только в приложении для компьютера");
  const tokens = await bridge.signIn(clientId);
  useSettings.getState().set({ spotifyAccount: { name: "Spotify", ...tokens } });
  const me = await spotifyMe();
  const account = useSettings.getState().spotifyAccount;
  if (account) useSettings.getState().set({ spotifyAccount: { ...account, ...me } });
}

export function signOutSpotify(): void {
  useSettings.getState().set({ spotifyAccount: null });
}

async function withProgress(label: string, job: (signal: AbortSignal, step: (text: string) => void) => Promise<void>): Promise<void> {
  const controller = new AbortController();
  const progress = useImportProgress.getState();
  progress.start(label, () => controller.abort());
  try {
    await job(controller.signal, (text) => progress.update(text));
    progress.finish(controller.signal.aborted ? { ok: false, text: "Загрузка отменена" } : { ok: true, text: "Готово · импорт закончен" });
  } catch (e) {
    const message = e instanceof Error ? e.message : String(e);
    if (!controller.signal.aborted) toast(message, "error");
    progress.finish({ ok: false, text: controller.signal.aborted ? "Загрузка отменена" : `Не загрузилось: ${message}` });
  }
}

export function importSpotifyLiked(): Promise<void> {
  return withProgress("Spotify: читаю «Любимые треки»…", async (signal, step) => {
    const named = await spotifyLikedSongs(signal, (n, total) => step(`Spotify: прочитано ${n} из ${total}`));
    if (!named.length) throw new Error("В «Любимых треках» Spotify пусто");
    const tracks = await matchTracks(named, { signal, onProgress: (p) => step(`Любимые Spotify: ${p.done} из ${p.total}, найдено ${p.found}`) });
    if (signal.aborted) return;
    const added = useLibrary.getState().addLikes(tracks);
    toast(`В понравившиеся добавлено ${added} из ${named.length}`);
  });
}

export function importSpotifyPlaylists(): Promise<void> {
  return withProgress("Spotify: читаю плейлисты…", async (signal, step) => {
    const lists = await spotifyMyPlaylists(signal);
    let created = 0;
    for (let i = 0; i < lists.length && !signal.aborted; i++) {
      const p = lists[i];
      const source = `spotify:${p.id}`;
      const { tracks: named } = await spotifyCollection("playlist", p.id, signal, (n, total) => step(`${i + 1}/${lists.length} «${p.name}»: читаю ${n} из ${total}`)).catch(() => ({ tracks: [] }));
      const tracks = await matchTracks(named, { signal, onProgress: (x) => step(`${i + 1}/${lists.length} «${p.name}»: ${x.done} из ${x.total}, найдено ${x.found}`) });
      if (!tracks.length || signal.aborted) continue;
      const lib = useLibrary.getState();
      const existing = lib.playlists.find((pl) => pl.source === source);
      if (existing) lib.addToPlaylist(existing.id, tracks);
      else {
        const pl = lib.createPlaylist(p.name, "Из вашего Spotify", source);
        if (p.artwork) lib.updatePlaylist(pl.id, { icon: p.artwork });
        lib.addToPlaylist(pl.id, tracks);
        created++;
      }
    }
    if (!signal.aborted) toast(lists.length ? `Плейлистов из Spotify: новых ${created}` : "Плейлистов не нашлось");
  });
}
