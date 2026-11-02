// „Nicht jetzt“ der Setup-Dialoge gilt bis zum nächsten Start der App: gemerkt in sessionStorage.
// Ohne Speicher (privater Modus, Tests ohne DOM) zählt nichts als abgewiesen.

const PLUGIN_KEY = 'nexus.setup.pluginLater';
const MIGRATION_KEY = 'nexus.setup.migrationLater';

export function pluginLater(): boolean {
  try {
    return sessionStorage.getItem(PLUGIN_KEY) === '1';
  } catch {
    return false;
  }
}

export function rememberPluginLater(): void {
  try {
    sessionStorage.setItem(PLUGIN_KEY, '1');
  } catch {
    // ohne Speicher kommt der Dialog beim nächsten Laden wieder
  }
}

export function migrationLater(boardId: number): boolean {
  return laterBoards().includes(boardId);
}

export function rememberMigrationLater(boardId: number): void {
  try {
    sessionStorage.setItem(MIGRATION_KEY, JSON.stringify([...laterBoards(), boardId]));
  } catch {
    // wie oben
  }
}

function laterBoards(): number[] {
  try {
    const value: unknown = JSON.parse(sessionStorage.getItem(MIGRATION_KEY) ?? '[]');
    return Array.isArray(value) ? value.filter((id): id is number => typeof id === 'number') : [];
  } catch {
    return [];
  }
}
