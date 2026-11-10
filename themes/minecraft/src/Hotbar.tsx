// Hotbar mit 9 Feldern als Hauptnavigation: Klick, Tasten 1–9 und Mausrad wechseln die Seite wie im Spiel das Item
import { NavLink, useNav, useNavigate, type NavItem } from '@nexus/theme-sdk';
import { useEffect, useRef } from 'react';
import { McText } from './McText.js';
import { NavIcon } from './NavIcon.js';
import { isShortcut } from './keys.js';
import { useSound } from './useSound.js';

/** Mindestabstand zwischen zwei Wechseln per Mausrad (Touchpads feuern viele kleine Ereignisse) */
const WHEEL_PAUSE_MS = 150;

export function Hotbar() {
  const nav = useNav();
  const navigate = useNavigate();
  const bar = useRef<HTMLElement>(null);
  const play = useSound();

  // Feldwechsel mit Ton wie im Spiel; das schon aktive Feld bleibt still
  const select = (item: NavItem) => {
    if (!item.active) play('hotbar');
    navigate(item.href);
  };

  // Tasten 1–9 wählen das Feld, außer beim Tippen, mit Strg/Alt oder über einem Dialog
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (!/^[1-9]$/.test(event.key) || !isShortcut(event)) return;
      const item = nav[Number(event.key) - 1];
      if (!item) return;
      event.preventDefault();
      select(item);
    };
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, [nav, navigate, play]);

  // Mausrad über der Hotbar: nach unten das nächste, nach oben das vorige Feld, am Rand rundherum; die Seite scrollt dabei nicht
  useEffect(() => {
    const element = bar.current;
    if (!element || nav.length === 0) return;
    let last = 0;
    const onWheel = (event: WheelEvent) => {
      if (event.deltaY === 0) return;
      event.preventDefault();
      const now = Date.now();
      if (now - last < WHEEL_PAUSE_MS) return;
      last = now;
      const step = event.deltaY > 0 ? 1 : -1;
      const index = nav.findIndex((item) => item.active);
      const from = index === -1 ? (step > 0 ? -1 : nav.length) : index;
      select(nav[(from + step + nav.length) % nav.length]);
    };
    element.addEventListener('wheel', onWheel, { passive: false });
    return () => element.removeEventListener('wheel', onWheel);
  }, [nav, navigate, play]);

  return (
    <>
      {/* Platz für die XP-Leiste über der Hotbar; Phase 4 füllt ihn mit der Claude-Nutzung */}
      <div className="mc-xp-slot" aria-hidden="true" />
      <nav ref={bar} className="mc-hotbar" aria-label="Hauptnavigation">
        {nav.map((item) => (
          <NavLink key={item.id} id={item.id} className={`mc-hotbar-slot${item.active ? ' mc-hotbar-slot--selected' : ''}`}
            onClick={() => {
              if (!item.active) play('hotbar');
            }}
          >
            {() => (
              <>
                <NavIcon name={item.id} />
                {item.badge ? <span className="mc-hotbar-count">{item.badge}</span> : null}
                <span className="mc-hotbar-label">
                  <McText>{item.label}</McText>
                </span>
              </>
            )}
          </NavLink>
        ))}
      </nav>
    </>
  );
}
