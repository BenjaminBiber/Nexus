import { useEffect, type ReactNode } from 'react';
import type { ShellProps } from '@nexus/theme-sdk';
import { GameProvider, useGame } from './data/GameContext.js';
import { SaShell } from './sa/SaShell.js';
import { rememberBoot, rememberStyle, useGtaSettings, useRootAttributes } from './settings.js';
import { VShell } from './v/VShell.js';

// Rahmen des aktiven Stils (beim Laden der gemerkte); der GameProvider darüber bleibt beim Umschalten stehen
function Frame({ content }: { content: ReactNode }) {
  return useGame().style === 'sa' ? <SaShell content={content} /> : <VShell content={content} />;
}

// Rahmen im Schema „GTA“: der Spiel-Zustand (GameProvider) und darin der Rahmen des Stils. Stil, Akzent und Radar landen
// als data-gta-* auf <html>. Die Navigation baut der Rahmen selbst aus useNav(), das Fragment `nav` des Kerns bleibt ungenutzt.
export function GtaShell({ pid, content }: ShellProps) {
  const settings = useGtaSettings();
  useRootAttributes(settings);

  // Erst nach dem Laden merken, sonst überschrieben die Defaults den gemerkten Stil
  useEffect(() => {
    if (settings.loading) return;
    rememberStyle(settings.style);
    rememberBoot(settings.boot);
  }, [settings.loading, settings.style, settings.boot]);

  return (
    <GameProvider pid={pid}>
      <Frame content={content} />
    </GameProvider>
  );
}
