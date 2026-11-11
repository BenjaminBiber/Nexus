import { useEffect } from 'react';
import type { ShellProps } from '@nexus/theme-sdk';
import { GameProvider } from './data/GameContext.js';
import { rememberBoot, rememberStyle, useGtaSettings, useRootAttributes } from './settings.js';
import { VShell } from './v/VShell.js';

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
      <VShell content={content} />
    </GameProvider>
  );
}
