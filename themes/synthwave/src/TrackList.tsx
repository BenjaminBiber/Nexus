import { NavLink, useNav } from '@nexus/theme-sdk';

// Titelnummer zweistellig: 1 → „01“
const trackNo = (index: number) => String(index + 1).padStart(2, '0');

// Navigation als Titelliste der A-Seite: je Eintrag ein Link des Hosts mit Titelnummer, Beschriftung und Zähler; der
// laufende Titel (aktiver Eintrag) trägt den Equalizer. Nummer und Equalizer sind Schmuck und für Screenreader verborgen.
export function TrackList() {
  const items = useNav();
  return (
    <nav className="nav synth-tracks" aria-label="Hauptnavigation">
      <div className="synth-side" aria-hidden="true">
        SIDE A
      </div>
      {items.map((entry, index) => (
        <NavLink key={entry.id} id={entry.id} className={`synth-track${entry.active ? ' active' : ''}`}>
          {(item) => (
            <>
              <span className="synth-track-no" aria-hidden="true">
                {trackNo(index)}
              </span>
              <span className="nav-label">{item.label}</span>
              {item.badge ? <span className="nav-badge">{item.badge}</span> : null}
              {item.active && (
                <span className="synth-eq" aria-hidden="true">
                  <i />
                  <i />
                  <i />
                </span>
              )}
            </>
          )}
        </NavLink>
      ))}
    </nav>
  );
}
