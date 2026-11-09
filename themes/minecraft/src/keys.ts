// Gemeinsame Prüfungen für die Tastenkürzel der Shell (1–9, Esc)

/** Tippt der Nutzer gerade in ein Feld? Dann gehören die Tasten dem Feld */
export function isEditable(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.isContentEditable || target.matches('input, textarea, select');
}

/** Liegt ein Dialog oder ein aufgeklapptes Menü bzw. eine Liste (z. B. die Suche) über der Seite oder um das Ziel? */
export function isOverlayOpen(target: EventTarget | null): boolean {
  if (target instanceof Element && target.closest('[role="dialog"], [role="alertdialog"], [role="menu"], [role="listbox"]')) return true;
  return document.querySelector('dialog[open], [aria-modal="true"]') !== null;
}

/** Ein Tastendruck, den die Shell als Kürzel auswerten darf */
export function isShortcut(event: KeyboardEvent): boolean {
  if (event.defaultPrevented || event.ctrlKey || event.altKey || event.metaKey) return false;
  return !isEditable(event.target) && !isOverlayOpen(event.target);
}
