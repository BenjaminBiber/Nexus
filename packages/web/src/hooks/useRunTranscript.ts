import { useEffect, useRef, useState } from 'react';
import { useApi } from '../api/ApiContext.js';
import type { DispatchRunShotDto } from '../api/dispatchTypes.js';
import type { TranscriptEntry } from '../api/sessionDetailTypes.js';
import { mergeEntries, TRANSCRIPT_PAGE_SIZE } from './useSessionDetail.js';

export interface RunTranscriptState {
  /** 'loading' bis zum ersten abgeschlossenen Abruf */
  status: 'loading' | 'ready';
  /** Es gibt ein Transcript zu diesem Lauf; sonst zeigt die Oberfläche das Lauf-Protokoll */
  available: boolean;
  entries: TranscriptEntry[];
  shots: DispatchRunShotDto[];
}

const INITIAL: RunTranscriptState = { status: 'loading', available: false, entries: [], shots: [] };
/** Mehr Seiten holt ein Abruf nicht (Schutz vor einer Schleife bei kaputtem Cursor) */
const MAX_PAGES = 40;

// Lädt Verlauf (seitenweise ab Cursor) und Screenshots eines Dispatcher-Laufs. Solange der Lauf läuft, wird alle
// `intervalMs` nachgeladen, nach dem Ende noch einmal. Ohne Transcript bleibt `available` false.
export function useRunTranscript(name: string, running: boolean, intervalMs = 3000): RunTranscriptState {
  const api = useApi();
  const [state, setState] = useState<RunTranscriptState>(INITIAL);
  // Der Abruf des aktuellen Laufs; der Zeitgeber unten ruft immer den gerade gültigen
  const pull = useRef<(() => Promise<void>) | null>(null);

  useEffect(() => {
    let active = true;
    let busy = false;
    let again = false;
    let cursor: string | null = null;
    let entries: TranscriptEntry[] = [];
    let shots: DispatchRunShotDto[] = [];
    let available = false;
    setState(INITIAL);

    // Ein Durchgang: Seiten ab dem Cursor, dann die Screenshots; der Zustand ändert sich einmal am Ende
    const once = async () => {
      try {
        for (let page = 0; page < MAX_PAGES; page += 1) {
          const next = await api.getDispatchRunTranscript(name, cursor, TRANSCRIPT_PAGE_SIZE);
          if (!active) return;
          if (!next.available) break;
          available = true;
          cursor = next.cursor || cursor;
          entries = mergeEntries(entries, next.entries);
          if (next.entries.length < TRANSCRIPT_PAGE_SIZE) break;
        }
      } catch {
        // Verlauf gerade nicht lesbar: der bisherige Stand bleibt, der nächste Durchgang versucht es erneut
      }
      if (!active) return;
      try {
        const list = await api.getDispatchRunShots(name);
        if (!active) return;
        shots = list;
      } catch {
        // Screenshots gerade nicht lesbar: die bisherige Liste bleibt
      }
      if (active) setState({ status: 'ready', available, entries, shots });
    };

    // Abrufe überlappen nicht; kommt einer dazwischen, wird danach einmal nachgeholt
    const load = async () => {
      if (busy) {
        again = true;
        return;
      }
      busy = true;
      try {
        do {
          again = false;
          await once();
        } while (again && active);
      } finally {
        busy = false;
      }
    };

    pull.current = load;
    void load();
    return () => {
      active = false;
      pull.current = null;
    };
  }, [api, name]);

  // Solange der Lauf läuft, regelmäßig nachladen; nach dem Ende noch einmal (die letzten Einträge)
  const wasRunning = useRef(false);
  useEffect(() => {
    if (running) {
      wasRunning.current = true;
      const timer = setInterval(() => void pull.current?.(), intervalMs);
      return () => clearInterval(timer);
    }
    if (wasRunning.current) {
      wasRunning.current = false;
      void pull.current?.();
    }
    return undefined;
  }, [running, intervalMs]);

  return state;
}
