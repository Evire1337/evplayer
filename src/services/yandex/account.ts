import { useSettings } from "@/settings/store";
import { useLibrary } from "@/library/store";
import { matchTracks } from "@/library/matchTracks";
import { useImportProgress } from "@/library/importProgress";
import { toast } from "@/ui/toasts";
import { yandexCollection, yandexLikes, yandexProfile } from "./music";

// "Connect Yandex Music": a profile by login. Its public playlists and "Мне нравится" are
// carried over; every song is found on YouTube Music or SoundCloud.

export async function connectYandex(login: string): Promise<void> {
  const value = login.trim();
  const fromUrl = /music\.yandex\.[a-z]+\/users\/([^/?#]+)/.exec(value)?.[1];
  const profile = await yandexProfile(fromUrl ? decodeURIComponent(fromUrl) : value);
  useSettings.getState().set({ yandexAccount: { login: profile.login, name: profile.name, playlists: profile.playlists.length } });
}

export function disconnectYandex(): void {
  useSettings.getState().set({ yandexAccount: null });
}

async function withProgress(label: string, job: (signal: AbortSignal, step: (text: string) => void) => Promise<void>): Promise<void> {
  const controller = new AbortController();
  const progress = useImportProgress.getState();
  progress.start(label, () => controller.abort());
  try {
    await job(controller.signal, (text) => progress.update(text));
  } catch (e) {
    if (!controller.signal.aborted) toast(e instanceof Error ? e.message : String(e), "error");
  } finally {
    progress.finish();
  }
}

export function importYandexLikes(): Promise<void> {
  const account = useSettings.getState().yandexAccount;
  if (!account) return Promise.resolve();
  return withProgress("Яндекс Музыка: читаю «Мне нравится»…", async (signal, step) => {
    const named = await yandexLikes(account.login, signal);
    if (!named.length) throw new Error("«Мне нравится» пусто или закрыто — откройте его в настройках приватности Яндекс Музыки");
    const tracks = await matchTracks(named, { signal, onProgress: (p) => step(`Яндекс «Мне нравится»: ${p.done} из ${p.total}, найдено ${p.found}`) });
    if (signal.aborted) return;
    const added = useLibrary.getState().addLikes(tracks);
    toast(`В понравившиеся добавлено ${added} из ${named.length}`);
  });
}

export function importYandexPlaylists(): Promise<void> {
  const account = useSettings.getState().yandexAccount;
  if (!account) return Promise.resolve();
  return withProgress("Яндекс Музыка: читаю плейлисты…", async (signal, step) => {
    const profile = await yandexProfile(account.login, signal);
    let created = 0;
    for (let i = 0; i < profile.playlists.length && !signal.aborted; i++) {
      const p = profile.playlists[i];
      const source = `ym:${profile.login}/${p.id}`;
      const { tracks: named } = await yandexCollection({ kind: "playlist", owner: profile.login, id: p.id }, signal);
      const tracks = await matchTracks(named, {
        signal,
        onProgress: (x) => step(`${i + 1}/${profile.playlists.length} «${p.title}»: ${x.done} из ${x.total}, найдено ${x.found}`),
      });
      if (!tracks.length || signal.aborted) continue;
      const lib = useLibrary.getState();
      const existing = lib.playlists.find((pl) => pl.source === source);
      if (existing) lib.addToPlaylist(existing.id, tracks);
      else {
        const pl = lib.createPlaylist(p.title, "Из вашей Яндекс Музыки", source);
        if (p.cover) lib.updatePlaylist(pl.id, { icon: p.cover });
        lib.addToPlaylist(pl.id, tracks);
        created++;
      }
    }
    if (!signal.aborted) toast(profile.playlists.length ? `Плейлистов из Яндекс Музыки: новых ${created}` : "Публичных плейлистов не нашлось");
  });
}
