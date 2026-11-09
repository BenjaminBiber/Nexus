// Slot „Shell“ im Stil des Minecraft-Titelbildschirms: Panorama im Hintergrund, Kopfzeile mit den Bausteinen des Hosts,
// die Seite in einem Panel, unten die Hotbar als Hauptnavigation; „Menü“ bzw. Esc öffnet das Titelmenü.
import { BoardSwitcher, ConnectionStatus, Outlet, SearchButton, UsageMeter, useNav, type ShellProps } from '@nexus/theme-sdk';
import { useEffect, useRef, useState } from 'react';
import { McText } from './BitmapText.js';
import { ResourcesProvider } from './context.js';
import { Hotbar } from './Hotbar.js';
import { LoadingGate } from './LoadingGate.js';
import { McButton } from './McButton.js';
import { Panorama } from './Panorama.js';
import { TitleMenu } from './TitleMenu.js';
import { isShortcut } from './keys.js';
import { useMinecraftResources } from './resources.js';

export function MinecraftShell({ parts }: ShellProps) {
  const resources = useMinecraftResources();
  const active = useNav().find((item) => item.active);
  // Ohne offene Seite zeigt die Shell den Titelbildschirm
  const [menuOpen, setMenuOpen] = useState(!active);
  const menuButton = useRef<HTMLButtonElement>(null);
  const title = active?.label ?? 'Nexus';

  // Wechsel des Ziels schließt das Menü
  useEffect(() => setMenuOpen(!active), [active?.id]);

  // Esc außerhalb von Feldern und Dialogen öffnet das Titelmenü wie im Spiel
  useEffect(() => {
    if (menuOpen) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape' || !isShortcut(event)) return;
      event.preventDefault();
      setMenuOpen(true);
    };
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, [menuOpen]);

  const closeMenu = (focusMenuButton: boolean) => {
    setMenuOpen(false);
    if (focusMenuButton) menuButton.current?.focus();
  };

  return (
    <ResourcesProvider urls={resources.urls}>
      <LoadingGate resources={resources}>
        <Panorama />
        <header className="mc-header">
          <McButton
            ref={menuButton}
            className="mc-menu-button"
            aria-haspopup="menu"
            aria-expanded={menuOpen}
            onClick={() => (menuOpen ? closeMenu(false) : setMenuOpen(true))}
          >
            Menü
          </McButton>
          <McText className="mc-header-title">Nexus</McText>
          <div className="mc-header-parts">
            <SearchButton />
            <BoardSwitcher />
            <UsageMeter />
            <ConnectionStatus />
          </div>
        </header>
        {menuOpen && <TitleMenu onClose={closeMenu} />}
        {/* Bleibt beim offenen Menü eingehängt, damit die Seite ihren Zustand behält */}
        <section className="mc-panel" aria-label={title} hidden={menuOpen}>
          <h2 className="mc-panel-title">
            <McText>{title}</McText>
          </h2>
          {parts.connection}
          <div className="mc-panel-body">
            <Outlet />
          </div>
        </section>
        <Hotbar />
      </LoadingGate>
    </ResourcesProvider>
  );
}
