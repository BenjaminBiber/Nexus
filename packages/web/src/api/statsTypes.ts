/** Art der ausgewerteten Läufe */
export type DispatchRunKindDto = 'work' | 'merge' | 'research';

/** Ausgänge der Läufe eines Profils: gelöst, abgebrochen mit weiterem Versuch, endgültig gescheitert, Rest */
export interface ProfileRowDto {
  profile: string;
  runs: number;
  done: number;
  escalated: number;
  final: number;
  other: number;
}

/** Abschnitt der Zeitreihe; `start` = JJJJ-MM-TT (Tag bzw. Montag der Woche); `other` = Frage oder Blockiert */
export interface SeriesPointDto { start: string; done: number; aborted: number; other: number }

/** Antwort von GET /api/db/stats/dispatch */
export interface DispatchStatsDto {
  filter: { project: string; boardId: number | null; from: string; to: string; kind: DispatchRunKindDto };
  kind: DispatchRunKindDto;
  /** Ältester vom Dispatcher erfasster Lauf im Projekt; null = Erfassung lief noch nie */
  runsSince: string | null;
  backfilledRuns: number;
  /** Erstes Auftreten der neuesten Fassung der Dispatcher-Einstellung (ISO) */
  configChangedAt: string | null;
  kpis: {
    runs: number;
    subjects: number;
    done: number;
    aborted: number;
    /** Tickets bzw. Fragen mit einem erfolgreichen Lauf */
    solved: number;
    /** Gelöste / abgeschlossene Tickets bzw. Fragen (gelöst oder endgültig gescheitert) */
    successRate: number | null;
    firstTryRate: number | null;
    escalationRate: number | null;
    finalFailures: number;
  };
  /** Läufe im Zeitverlauf: bis 28 Tage je Tag, darüber je Woche (Montag) */
  series: { unit: 'day' | 'week'; points: SeriesPointDto[] };
  byProfile: ProfileRowDto[];
  /** Abgeschlossene Tickets: Anteile im ersten Versuch gelöst, nach Eskalation gelöst, endgültig gescheitert */
  ladder: { finished: number; firstTry: number | null; afterEscalation: number | null; final: number | null };
  abortReasons: Array<{ category: string; label: string; count: number }>;
  /** Anteil der Abbrüche an Schritt-, Zeitlimit oder vollem Kontext */
  configurableAbortShare: number | null;
  usagePauses: number;
  staleRuns: number;
}

/** Filter der Auswertung; Tage als JJJJ-MM-TT (Ortszeit), from = 'config': seit der letzten Änderung der Einstellung */
export interface DispatchStatsFilter {
  project: string;
  board?: number | null;
  from?: string;
  to?: string;
  kind?: DispatchRunKindDto;
}
