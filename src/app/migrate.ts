// The first start after the move from the Electron version: its saved state (library,
// playlists, settings, statistics) is read by the desktop side from the old app's storage
// and written here, then the page reloads with it. Runs once; never over existing data.

const DONE = "evplayer.migrated";

export async function migrateFromElectron(): Promise<boolean> {
  const api = window.evplayer;
  if (!api || localStorage.getItem(DONE)) return false;
  let data: Record<string, string> | null = null;
  try {
    data = await api.migrateFromElectron();
  } catch (error) {
    console.warn("[migrate]", error);
  }
  localStorage.setItem(DONE, new Date().toISOString());
  if (!data) return false;
  const ownData = Object.keys(localStorage).some((k) => k.startsWith("evplayer.") && k !== DONE);
  if (ownData) return false;
  for (const [key, value] of Object.entries(data)) localStorage.setItem(key, value);
  location.reload();
  return true;
}
