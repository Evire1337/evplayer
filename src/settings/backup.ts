// A backup is every EvPlayer setting and list kept on this computer (likes, playlists,
// statistics, history, equalizer presets, settings) in one JSON file. Your own audio files
// are not inside, only the list of them stays with the app.

const PREFIX = "evplayer.";
// caches that rebuild themselves are left out
const SKIP = [/^evplayer\.loudness/];

export interface BackupFile {
  app: "EvPlayer";
  version: 1;
  createdAt: string;
  data: Record<string, string>;
}

export function makeBackup(storage: Storage = localStorage, now = new Date()): BackupFile {
  const data: Record<string, string> = {};
  for (let i = 0; i < storage.length; i++) {
    const key = storage.key(i);
    if (!key || !key.startsWith(PREFIX) || SKIP.some((re) => re.test(key))) continue;
    const value = storage.getItem(key);
    if (value !== null) data[key] = value;
  }
  return { app: "EvPlayer", version: 1, createdAt: now.toISOString(), data };
}

export function readBackup(text: string): BackupFile {
  const parsed = JSON.parse(text) as BackupFile;
  if (parsed?.app !== "EvPlayer" || parsed.version !== 1 || typeof parsed.data !== "object") throw new Error("Это не файл резервной копии EvPlayer");
  return parsed;
}

/** replace what is on this computer with the backup; the page reloads afterwards */
export function restoreBackup(backup: BackupFile, storage: Storage = localStorage): number {
  const keys: string[] = [];
  for (let i = 0; i < storage.length; i++) {
    const key = storage.key(i);
    if (key?.startsWith(PREFIX)) keys.push(key);
  }
  for (const key of keys) storage.removeItem(key);
  let n = 0;
  for (const [key, value] of Object.entries(backup.data)) {
    if (!key.startsWith(PREFIX) || typeof value !== "string") continue;
    storage.setItem(key, value);
    n++;
  }
  return n;
}

export async function saveBackupFile(): Promise<boolean> {
  const backup = makeBackup();
  const name = `evplayer-backup-${backup.createdAt.slice(0, 10)}.json`;
  const text = JSON.stringify(backup);
  if (window.evplayer) return window.evplayer.files.saveText(name, text);
  const a = document.createElement("a");
  a.href = URL.createObjectURL(new Blob([text], { type: "application/json" }));
  a.download = name;
  a.click();
  URL.revokeObjectURL(a.href);
  return true;
}
