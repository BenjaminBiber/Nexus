import { useEffect } from 'react';
import { BoardSwitcher, SearchButton, useNav, type ShellProps } from '@nexus/theme-sdk';
import { useRootAttributes, useSynthSettings } from './settings.js';
import { TrackList } from './TrackList.js';
import { VuMeter } from './VuMeter.js';
import { OnAir } from './OnAir.js';
import { Horizon } from './Horizon.js';
import { useSkyPhase } from './sky.js';
import { rememberBoot, rememberPalette } from './SunriseDrive.js';

// Rahmen im Schema „Synthwave“ als Kassettendeck: oben das Display mit Schriftzug, Board-Auswahl und Suche, links die
// Navigation als Titelliste der A-Seite, rechts die Seite und unten die Deck-Leiste mit dem Etikett des laufenden Titels.
// Hinter dem Inhalt liegt der Horizont; die Einstellungen und die Phase des Himmels landen als data-synth-* auf <html>.
export function SynthShell({ content }: ShellProps) {
  const active = useNav().find((item) => item.active);
  const settings = useSynthSettings();
  const sky = useSkyPhase(settings.sky);
  useRootAttributes(settings, sky);

  // Startanimation läuft vor der Shell: Schalter und Farbschema für den nächsten Start merken
  useEffect(() => {
    if (settings.loading) return;
    rememberBoot(settings.boot);
    rememberPalette(settings.palette);
  }, [settings.loading, settings.boot, settings.palette]);

  return (
    <div className="synth-screen">
      <header className="synth-display">
        <span className="synth-logo synth-script" aria-hidden="true">
          Nexus
        </span>
        <div className="synth-board">
          <BoardSwitcher />
        </div>
        <div className="synth-search">
          <SearchButton />
        </div>
      </header>
      <div className="synth-body">
        {settings.horizon !== 'off' && <Horizon />}
        <TrackList />
        {content}
      </div>
      <footer className="synth-deck" data-section={active?.id ?? 'board'}>
        <span className="synth-tape-label">{active?.label ?? ''}</span>
        <VuMeter />
        <span className="synth-spacer" />
        <OnAir />
      </footer>
    </div>
  );
}
