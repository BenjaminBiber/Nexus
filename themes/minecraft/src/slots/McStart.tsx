// Slot „StartAnimation“: bis Phase 6 nur ein Platzhalter, der den Start sofort freigibt.
// Er steht schon jetzt im Manifest, damit die Slot-Liste des Themes endgültig ist.
import { useEffect, useRef } from 'react';
import type { StartAnimationProps } from '@nexus/theme-sdk';

export function McStart({ onDone }: StartAnimationProps) {
  // Nur beim ersten Erscheinen melden, auch wenn React die Komponente doppelt einhängt (Strict Mode)
  const done = useRef(false);
  useEffect(() => {
    if (done.current) return;
    done.current = true;
    onDone();
  }, [onDone]);
  return null;
}
