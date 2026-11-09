// Rahmen der Minecraft-Shell: Fortschritt bzw. Hinweis oben, die Seite darunter bleibt immer bedienbar
import type { CSSProperties, ReactNode } from 'react';
import {
  BUTTON,
  BUTTON_HIGHLIGHTED,
  FONT_ATLAS,
  FOOTER_SEPARATOR,
  HEADER_SEPARATOR,
  HOTBAR,
  HOTBAR_SELECTION,
  MENU_BACKGROUND,
  MENU_LIST_BACKGROUND,
  PANORAMA_OVERLAY,
  panorama,
  type MinecraftResources,
} from './resources.js';

const OFFLINE_HINT =
  'Keine Verbindung zu Mojang. Die Ressourcen werden beim ersten Start einmal geladen. Bitte Internetverbindung prüfen und erneut versuchen.';

// CSS-Variablen auf die Proxy-URLs; fehlt eine, greifen die Rückfall-Farben aus theme.css
function cssVariables(urls: Record<string, string>): CSSProperties {
  const vars: Record<string, string> = {};
  const set = (name: string, entry: string) => {
    if (urls[entry]) vars[name] = `url("${urls[entry]}")`;
  };
  set('--mc-font-atlas', FONT_ATLAS);
  set('--mc-button', BUTTON);
  set('--mc-button-highlighted', BUTTON_HIGHLIGHTED);
  for (let i = 0; i < 6; i++) set(`--mc-panorama-${i}`, panorama(i));
  set('--mc-panorama-overlay', PANORAMA_OVERLAY);
  set('--mc-hotbar', HOTBAR);
  set('--mc-hotbar-selection', HOTBAR_SELECTION);
  set('--mc-menu-bg', MENU_BACKGROUND);
  set('--mc-list-bg', MENU_LIST_BACKGROUND);
  set('--mc-sep-top', HEADER_SEPARATOR);
  set('--mc-sep-bottom', FOOTER_SEPARATOR);
  return vars as CSSProperties;
}

export function LoadingGate({ resources, children }: { resources: MinecraftResources; children: ReactNode }) {
  const { phase, done, total, urls, error, retry } = resources;
  return (
    <div data-testid="mc-shell" className={`mc-shell${phase === 'ready' ? ' mc-ready' : ''}`} style={cssVariables(urls)}>
      {phase !== 'ready' && phase !== 'error' && (
        <div data-testid="mc-loading" className="mc-status" role="status">
          <span>
            Minecraft-Ressourcen werden geladen … {done}/{total}
          </span>
          <progress value={done} max={total} />
        </div>
      )}
      {phase === 'error' && error && (
        <div data-testid="mc-error" className="mc-status mc-error" role="alert">
          <span>{error.offline ? OFFLINE_HINT : `Minecraft-Ressourcen konnten nicht geladen werden. ${error.message}`}</span>
          <button type="button" onClick={retry}>
            Erneut versuchen
          </button>
        </div>
      )}
      {children}
    </div>
  );
}
