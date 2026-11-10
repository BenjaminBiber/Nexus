import { BoardSwitcher, SearchButton, useNav, type ShellProps } from '@nexus/theme-sdk';
import { useRootAttributes, useSynthSettings } from './settings.js';
import { TrackList } from './TrackList.js';

// Rahmen im Schema „Synthwave“ als Kassettendeck: oben das Display mit Schriftzug, Board-Auswahl und Suche, links die
// Navigation als Titelliste der A-Seite, rechts die Seite und unten die Deck-Leiste mit dem Etikett des laufenden Titels.
// Die Einstellungen des Themes landen als data-synth-* auf <html>.
export function SynthShell({ content }: ShellProps) {
  const active = useNav().find((item) => item.active);
  const settings = useSynthSettings();
  useRootAttributes(settings);

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
        <TrackList />
        {content}
      </div>
      <footer className="synth-deck" data-section={active?.id ?? 'board'}>
        <span className="synth-tape-label">{active?.label ?? ''}</span>
      </footer>
    </div>
  );
}
