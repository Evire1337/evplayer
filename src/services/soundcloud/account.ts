import { useSettings } from "@/settings/store";
import { useLibrary } from "@/library/store";
import { resolveSoundCloudUser, soundCloudCollection, soundCloudUserLikes, soundCloudUserPlaylists } from "./api";

// "Your SoundCloud account": EvPlayer reads the public side of your profile (likes,
// playlists) and keeps it in your library. No password is involved.

export async function connectSoundCloud(input: string): Promise<void> {
  const user = await resolveSoundCloudUser(input);
  useSettings.getState().set({
    soundcloudAccount: {
      id: user.id,
      name: user.name,
      avatar: user.avatar,
      permalink: user.permalink,
      followers: user.followers,
      likes: user.likes,
      tracks: user.tracks,
      syncLikes: true,
    },
  });
}

export function disconnectSoundCloud(): void {
  useSettings.getState().set({ soundcloudAccount: null });
}

function stamp(): void {
  const account = useSettings.getState().soundcloudAccount;
  if (account) useSettings.getState().set({ soundcloudAccount: { ...account, lastSync: new Date().toISOString() } });
}

/** your public likes on SoundCloud into "Liked"; returns how many were new */
export async function importSoundCloudLikes(signal?: AbortSignal): Promise<number> {
  const account = useSettings.getState().soundcloudAccount;
  if (!account) return 0;
  const tracks = await soundCloudUserLikes(account.id, signal);
  const added = useLibrary.getState().addLikes(tracks);
  stamp();
  return added;
}

/** your SoundCloud playlists as EvPlayer playlists; importing again adds new songs */
export async function importSoundCloudPlaylists(onProgress?: (done: number, total: number) => void, signal?: AbortSignal): Promise<{ created: number; updated: number }> {
  const account = useSettings.getState().soundcloudAccount;
  if (!account) return { created: 0, updated: 0 };
  const lists = await soundCloudUserPlaylists(account.id, signal);
  let created = 0;
  let updated = 0;
  for (let i = 0; i < lists.length; i++) {
    onProgress?.(i, lists.length);
    const summary = lists[i];
    const { tracks } = await soundCloudCollection(summary.id, signal);
    const lib = useLibrary.getState();
    const existing = lib.playlists.find((p) => p.source === summary.id);
    if (existing) {
      if (lib.addToPlaylist(existing.id, tracks) > 0) updated++;
    } else if (tracks.length) {
      const p = lib.createPlaylist(summary.title, "Из вашего SoundCloud", summary.id);
      if (summary.artwork) lib.updatePlaylist(p.id, { icon: summary.artwork });
      lib.addToPlaylist(p.id, tracks);
      created++;
    }
  }
  stamp();
  return { created, updated };
}

/** on start: pick up likes added on SoundCloud since last time */
export function syncSoundCloudOnStart(): void {
  const account = useSettings.getState().soundcloudAccount;
  if (!account?.syncLikes) return;
  const last = account.lastSync ? Date.parse(account.lastSync) : 0;
  if (Date.now() - last < 60 * 60_000) return;
  setTimeout(() => void importSoundCloudLikes().catch(() => {}), 8000);
}
