import type { StartAnimationProps } from '@nexus/theme-sdk';
import { Wordmark } from '../shared/Wordmark.js';
import { useBoot, useTip } from '../shared/useBoot.js';
import { SaSceneArt } from './SaScene.js';

/** Tipps im Ton von San Andreas; einer erscheint zufällig */
export const TIPS = [
  'Erledigte Aufträge bringen Respekt. Respekt bringt Fortschritt.',
  'Jedes blockierte Ticket ist ein Fahndungsstern. Lös den Blocker, dann verschwindet er.',
  'Alles, was der Agent tun musste, war dem verdammten Plan zu folgen.',
  'Klingelt das Handy, hat ein Agent eine Frage. Y nimmt an, N lässt sie liegen.',
  'Das Radar zeigt jedes offene Ticket als Blip in der Farbe seiner Spalte.',
  'Geld gibt es für abgenommene Tickets, nicht für angefangene.',
  'Unter „Stats“ steht, wie viel Respekt du dir auf diesem Board schon verdient hast.',
];

// Ladebildschirm im Stil San Andreas: der Vorort bei Sonnenuntergang, langsam herangezoomt, unten rechts die Wortmarke
// mit „Board“ in der gotischen Schrift, unten links ein Tipp und der Ladebalken. Ablauf, Abbruch und Schalter: useBoot.
export function SaBoot({ onDone }: StartAnimationProps) {
  const enabled = useBoot(onDone);
  const tip = useTip(TIPS);
  if (!enabled) return null;
  return (
    <div className="gta-boot gta-sa-boot" data-gta-part="boot">
      <SaSceneArt className="gta-sa-boot-scene" />
      <div className="gta-sa-boot-vignette" />
      <div className="gta-sa-boot-logo">
        <Wordmark name="nexus" label="Nexus" />
        <em>Board</em>
      </div>
      <div className="gta-sa-boot-foot">
        <p className="gta-sa-boot-tip">{tip}</p>
        <div className="gta-sa-boot-bar" aria-hidden="true">
          <i />
        </div>
      </div>
    </div>
  );
}
