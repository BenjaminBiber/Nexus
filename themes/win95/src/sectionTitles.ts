import type { NavIconName } from '@nexus/theme-sdk';

const SECTION_TITLES: Partial<Record<NavIconName, string>> = {
  board: 'Board',
  inbox: 'Meine Aufgaben',
  dashboard: 'Dashboard',
  sessions: 'Sessions',
  questions: 'Fragen',
  docs: 'Docs',
  guide: 'Anleitung',
  settings: 'Einstellungen',
};

// Fenster- und Tasktitel aus dem aktiven Eintrag der Navigation; ohne Eintrag „Nexus“
export function windowTitle(id?: NavIconName): string {
  return (id && SECTION_TITLES[id]) ?? 'Nexus';
}
