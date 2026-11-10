// Einstieg des Themes: Vite baut daraus dist/main.js (ESM); React und das SDK liefert Nexus zur Laufzeit
import { SearchButton, type ShellProps, type ThemeDefinition } from '@nexus/theme-sdk';

// Beispiel-Shell: Kopfzeile mit Suche (SDK-Baustein), Board-Auswahl und Nutzung, Navigation links, aktuelle Seite rechts
function Shell({ nav, content, parts }: ShellProps) {
  return (
    <div data-testid="template-shell" className="template-shell">
      <header className="template-shell-header">
        <strong>Mein Theme</strong>
        <SearchButton />
        {parts.boardSwitcher}
        {parts.usage}
      </header>
      <nav aria-label="Navigation" className="template-shell-nav">
        {nav}
      </nav>
      <main className="template-shell-main">{content}</main>
    </div>
  );
}

// id wie in manifest.json; nur Slots, die hier stehen, ersetzen die Standard-Komponenten
const theme: ThemeDefinition = { id: 'my-theme', slots: { Shell } };

export default theme;
