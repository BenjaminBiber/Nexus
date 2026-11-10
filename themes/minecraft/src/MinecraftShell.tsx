// Slot „Shell“ wie ein Untermenü der Java Edition: Panorama im Hintergrund, Kopfzeile mit Knopf „Menü“, Seitentitel und
// den Bausteinen des Hosts, darunter die Seite auf der Erde-Fläche zwischen zwei Trennlinien, im Fuß die Hotbar als
// Hauptnavigation; „Menü“ bzw. Esc öffnet das Titelmenü.
import { BoardSwitcher, ConnectionStatus, Outlet, SearchButton, UsageMeter, useNav, type ShellProps } from '@nexus/theme-sdk';
import { useEffect, useRef, useState } from 'react';
import { ResourcesProvider } from './context.js';
import { Hotbar } from './Hotbar.js';
import { LoadingGate } from './LoadingGate.js';
import { McButton } from './McButton.js';
import { McText } from './McText.js';
import { Panorama } from './Panorama.js';
import { TitleMenu } from './TitleMenu.js';
import { useMinecraftFont } from './font/useMinecraftFont.js';
import { isShortcut } from './keys.js';
import { useMinecraftResources } from './resources.js';

export function MinecraftShell({ parts }: ShellProps) {
  const resources = useMinecraftResources();
  // Die Spielschrift wird einmal aus den Atlanten gebaut (oder aus der Cache API geholt) und als Web-Schrift registriert
  const fontState = useMinecraftFont(resources.urls);
  const active = useNav().find((item) => item.active);
  // Ohne offene Seite zeigt die Shell den Titelbildschirm
  const [menuOpen, setMenuOpen] = useState(!active);
  // Ton beim Öffnen nur, wenn der Nutzer das Menü öffnet, nicht beim Titelbildschirm ohne Seite
  const [menuSound, setMenuSound] = useState(false);
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
      openMenu();
    };
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, [menuOpen]);

  const openMenu = () => {
    setMenuSound(true);
    setMenuOpen(true);
  };

  const closeMenu =(focusMenuButton: boolean) => {
    setMenuOpen(false);
    if (focusMenuButton) menuButton.current?.focus();
  };

  return (
    <ResourcesProvider urls={resources.urls} soundUrls={resources.soundUrls}>
      <LoadingGate resources={resources} fontState={fontState}>
        {/* Titelmenü offen = Titelbildschirm: scharf; eine Seite offen = Untermenü: unscharf */}
        <Panorama blur={!menuOpen} />
        {menuOpen && <TitleMenu onClose={closeMenu} sound={menuSound} />}
        {/* Bleibt beim offenen Menü eingehängt, damit die Seite ihren Zustand behält */}
        <div className="mc-screen" hidden={menuOpen}>
          <header className="mc-screen-header">
            <McButton
              ref={menuButton}
              className="mc-menu-button mc-button--half"
              aria-haspopup="menu"
              aria-expanded={menuOpen}
              onClick={openMenu}
            >
              Menü
            </McButton>
            <h1 className="mc-screen-title">
              <McText>{title}</McText>
            </h1>
            <div className="mc-header-parts">
              <SearchButton />
              <BoardSwitcher />
              <UsageMeter />
              <ConnectionStatus />
            </div>
          </header>
          <div className="mc-separator mc-separator--top" aria-hidden="true" />
          <main className="mc-list">
            {parts.connection}
            <Outlet />
          </main>
          <div className="mc-separator mc-separator--bottom" aria-hidden="true" />
        </div>
        <footer className="mc-footer">
          <Hotbar />
        </footer>
      </LoadingGate>
    </ResourcesProvider>
  );
}
