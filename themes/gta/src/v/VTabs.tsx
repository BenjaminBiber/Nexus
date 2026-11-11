import { useEffect, useRef } from 'react';
import { NavLink, useNav } from '@nexus/theme-sdk';

// Reiterzeile des Pausenmenüs: ein Reiter je Eintrag der Navigation, Zähler als rotes Kästchen. Bei wenig Platz scrollt
// die Zeile waagerecht; der aktive Reiter wird dann in den sichtbaren Bereich geholt.
export function VTabs() {
  const items = useNav();
  const ref = useRef<HTMLElement>(null);
  const active = items.find((item) => item.active)?.id;

  useEffect(() => {
    const tab = ref.current?.querySelector<HTMLElement>('[aria-current="page"]');
    tab?.scrollIntoView?.({ block: 'nearest', inline: 'nearest' });
  }, [active]);

  return (
    <nav ref={ref} className="gta-v-tabs" aria-label="Hauptnavigation">
      {items.map((entry) => (
        <NavLink key={entry.id} id={entry.id} className="gta-v-tab">
          {(item) => (
            <>
              <span className="gta-v-tab-label">{item.label}</span>
              {item.badge ? <b className="gta-v-badge">{item.badge}</b> : null}
            </>
          )}
        </NavLink>
      ))}
    </nav>
  );
}
