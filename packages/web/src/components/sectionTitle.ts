const SECTION_TITLES: Record<string, string> = {
  board: 'Board',
  inbox: 'Meine Aufgaben',
  dashboard: 'Dashboard',
  sessions: 'Sessions',
  questions: 'Fragen',
  docs: 'Docs',
  guide: 'Anleitung',
  settings: 'Einstellungen',
};

// Titel des Bereichs aus /p/:pid/<bereich>/… (Fenster- und Tasktitel im Schema „Windows 95“)
export function sectionTitle(pathname: string): string {
  const section = pathname.split('/')[3] ?? '';
  return SECTION_TITLES[section] ?? 'AI Harness';
}
