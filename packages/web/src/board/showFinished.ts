const KEY = 'harness.showFinishedSpecs';

// Liest, ob fertige Specs in den Auswahllisten angezeigt werden (Standard: nein, auch bei gesperrtem Speicher)
export function loadShowFinished(): boolean {
  try {
    return window.localStorage.getItem(KEY) === '1';
  } catch {
    return false;
  }
}

// Merkt die Einstellung; Fehler des Speichers werden bewusst ignoriert
export function saveShowFinished(value: boolean): void {
  try {
    window.localStorage.setItem(KEY, value ? '1' : '0');
  } catch {
    /* Speicher gesperrt: die Seite funktioniert auch ohne */
  }
}
