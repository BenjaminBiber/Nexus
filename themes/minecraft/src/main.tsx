// Einstieg des Minecraft-Themes: Vite baut daraus dist/main.js (ESM); React und das SDK liefert Nexus zur Laufzeit
import { SearchButton, type ShellProps, type ThemeDefinition } from '@nexus/theme-sdk';
import { LoadingGate } from './LoadingGate.js';
import { THEME_ID, useMinecraftResources } from './resources.js';

// Beispiel-Shell der Vorlage im Minecraft-Look; das Menü-Layout folgt später
function Shell({ nav, content, parts }: ShellProps) {
  const resources = useMinecraftResources();
  return (
    <LoadingGate resources={resources}>
      <header className="mc-shell-header">
        <strong>Nexus</strong>
        <SearchButton />
        {parts.boardSwitcher}
        {parts.usage}
      </header>
      <nav aria-label="Navigation" className="mc-shell-nav">
        {nav}
      </nav>
      <main className="mc-shell-main">{content}</main>
    </LoadingGate>
  );
}

const theme: ThemeDefinition = { id: THEME_ID, slots: { Shell } };

export default theme;
