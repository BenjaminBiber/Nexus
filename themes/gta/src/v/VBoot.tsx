import { useEffect, useRef, useState } from 'react';
import type { StartAnimationProps } from '@nexus/theme-sdk';
import { rememberedBoot } from '../settings.js';
import { Wordmark } from '../shared/Wordmark.js';
import { reducedMotion } from '../shared/motion.js';
import { SceneArt } from './VScene.js';

/** Dauer des Ladebildschirms (ms) */
export const BOOT_MS = 3200;

/** Tipps zum Board; einer erscheint zufällig */
export const TIPS = [
  'Jedes blockierte Ticket bringt dir einen Fahndungsstern ein.',
  'Mit Q und E wechselst du zwischen den Reitern, Strg und K öffnet die Suche.',
  'Ein erledigtes Ticket bringt $40.000, jeder erledigte Schritt $2.500.',
  'Der grüne Balken unter der Minimap ist der Fortschritt des Boards, der blaue und der gelbe sind die Kontingente von Claude.',
  'Fragen der Agenten kommen als Nachricht aufs Handy. Wer schnell entscheidet, hält die Agenten in Bewegung.',
  'Ein Klick auf die Minimap bringt dich zum Board zurück.',
  'Unter „Stats“ steht, welche Tickets als Nächstes dran sind.',
];

// Ladebildschirm im Stil GTA V: zwei schräg geteilte Bildflächen mit der Abendszene in Rasterpunkten, unten links die
// Wortmarke, rechts ein Tipp und der Lade-Kreis. Läuft vor dem Rahmen und außerhalb des Hosts; der Schalter kommt deshalb
// aus localStorage. Jede Taste oder ein Klick beendet ihn; ausgeschaltet oder bei reduzierter Bewegung endet er unsichtbar.
export function VBoot({ onDone }: StartAnimationProps) {
  const [enabled] = useState(() => rememberedBoot() && !reducedMotion());
  const [tip] = useState(() => TIPS[Math.min(TIPS.length - 1, Math.floor(Math.random() * TIPS.length))]!);
  const done = useRef(false);

  useEffect(() => {
    const finish = () => {
      if (done.current) return;
      done.current = true;
      onDone();
    };
    if (!enabled) {
      finish();
      return;
    }
    const timer = window.setTimeout(finish, BOOT_MS);
    window.addEventListener('keydown', finish);
    window.addEventListener('pointerdown', finish);
    return () => {
      window.clearTimeout(timer);
      window.removeEventListener('keydown', finish);
      window.removeEventListener('pointerdown', finish);
    };
  }, [enabled, onDone]);

  if (!enabled) return null;
  return (
    <div className="gta-boot gta-v-boot" data-gta-part="boot">
      <div className="gta-v-boot-panel gta-v-boot-a">
        <SceneArt className="gta-v-boot-scene" />
        <div className="gta-v-boot-dots" />
      </div>
      <div className="gta-v-boot-panel gta-v-boot-b">
        <SceneArt className="gta-v-boot-scene" />
        <div className="gta-v-boot-dots" />
      </div>
      <div className="gta-v-boot-fade" />
      <div className="gta-v-boot-logo">
        <Wordmark name="nexus" label="Nexus" />
        <em>Board</em>
      </div>
      <div className="gta-v-boot-tip">
        <b>Tipp</b>
        <p>{tip}</p>
      </div>
      <div className="gta-v-boot-spin">
        <i aria-hidden="true" />
        Lade Board
      </div>
    </div>
  );
}
