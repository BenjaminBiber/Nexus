const key = (pid: string) => `harness.docsSeen.${pid}`;

// Liest die zuletzt gesehene Änderungszeit der Dokumente eines Projekts (0, wenn nichts gespeichert oder Speicher gesperrt)
export function loadDocsSeen(pid: string): number {
  try {
    const raw = window.localStorage.getItem(key(pid));
    const value = raw === null ? 0 : Number(raw);
    return Number.isFinite(value) ? value : 0;
  } catch {
    return 0;
  }
}

// Merkt die zuletzt gesehene Änderungszeit der Dokumente; Fehler des Speichers werden bewusst ignoriert
export function saveDocsSeen(pid: string, mtimeMs: number): void {
  try {
    window.localStorage.setItem(key(pid), String(mtimeMs));
  } catch {
    /* Speicher gesperrt: die Seite funktioniert auch ohne */
  }
}
