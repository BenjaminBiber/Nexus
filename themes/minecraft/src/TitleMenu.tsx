// Titelbildschirm: Logo und das Hauptmenü als große Minecraft-Knöpfe. Pfeiltasten wandern, Enter öffnet, Esc schließt.
import { NavLink, useNav, useOpenSearch } from '@nexus/theme-sdk';
import { useEffect, useRef, type KeyboardEvent } from 'react';
import { McText } from './BitmapText.js';
import { useResourceUrl } from './context.js';
import { LOGO } from './resources.js';
import { useSound } from './useSound.js';

export function TitleMenu({ onClose, sound = false }: { onClose: (focusMenuButton: boolean) => void; sound?: boolean }) {
  const menu = useRef<HTMLDivElement>(null);
  const nav = useNav();
  const openSearch = useOpenSearch();
  const logo = useResourceUrl(LOGO);
  const play = useSound();

  // Beim Öffnen Fokus auf den ersten Eintrag und, wenn vom Nutzer geöffnet, der Menü-Ton
  useEffect(() => {
    menu.current?.querySelector<HTMLElement>('[role="menuitem"]')?.focus();
    if (sound) play('menuOpen');
  }, []);

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const items = Array.from(menu.current?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? []);
    const index = items.indexOf(document.activeElement as HTMLElement);
    if (event.key === 'Escape') {
      event.preventDefault();
      onClose(true);
    } else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const step = event.key === 'ArrowDown' ? 1 : -1;
      items[(index + step + items.length) % items.length]?.focus();
    } else if (event.key === 'Home' || event.key === 'End') {
      event.preventDefault();
      items[event.key === 'Home' ? 0 : items.length - 1]?.focus();
    }
  };

  return (
    <div className="mc-title-screen">
      <div className="mc-title-logo">
        {logo ? <img className="mc-title-logo-image" src={logo} alt="" draggable={false} /> : null}
        <McText className="mc-title-edition">Nexus</McText>
      </div>
      <div ref={menu} className="mc-title-menu" role="menu" aria-label="Hauptmenü" onKeyDown={onKeyDown}>
        {nav.map((item) => (
          <NavLink key={item.id} id={item.id} role="menuitem" className="mc-button mc-button--wide" onClick={() => onClose(false)}>
            {() => <McText>{item.label}</McText>}
          </NavLink>
        ))}
        <button
          type="button"
          role="menuitem"
          className="mc-button mc-button--wide"
          onClick={() => {
            onClose(false);
            openSearch();
          }}
        >
          <McText>Suchen …</McText>
        </button>
      </div>
    </div>
  );
}
