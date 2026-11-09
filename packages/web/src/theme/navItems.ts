import type { NavIconName } from '@nexus/theme-sdk';

/** Einträge der Hauptnavigation in ihrer Reihenfolge; Ziel ist `/p/<pid>/<id>` */
export const NAV_ITEMS: ReadonlyArray<{ id: NavIconName; label: string }> = [
  { id: 'board', label: 'Board' },
  { id: 'inbox', label: 'Meine Aufgaben' },
  { id: 'dashboard', label: 'Dashboard' },
  { id: 'sessions', label: 'Sessions' },
  { id: 'questions', label: 'Fragen' },
  { id: 'docs', label: 'Docs' },
  { id: 'guide', label: 'Anleitung' },
  { id: 'settings', label: 'Einstellungen' },
];

// Beschriftung eines Eintrags der Hauptnavigation
export function navLabel(id: NavIconName): string {
  return NAV_ITEMS.find((item) => item.id === id)?.label ?? id;
}
