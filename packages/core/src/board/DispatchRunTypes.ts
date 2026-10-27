export type DispatchRunKind = 'work' | 'merge' | 'research';

/** Ergebnis eines Laufs als feste Kategorie */
export type DispatchRunOutcome =
  | 'done' | 'review' | 'human' | 'abort' | 'abort-final' | 'blocked' | 'question' | 'pause-limit' | 'pause-claude-missing'
  | 'setup-failed' | 'taken' | 'gone' | 'other' | 'merge-session' | 'research';

/** Warum das Basisprofil eines Laufs gewählt wurde: Standardprofil, Label profil:, Aufwand laut Plan, Regel, Einstellung research/planning */
export type DispatchProfileReason = 'default' | 'label' | 'plan' | 'rule' | 'setting';

/** Herkunft einer Lauf-Zeile: vom Dispatcher erfasst oder aus [dispatch]-Kommentaren rekonstruiert */
export type DispatchRunSource = 'dispatcher' | 'comments';

/** Ein ausgewerteter Lauf des Dispatchers (Tabelle board.DispatchRuns) */
export interface DispatchRun {
  /** Lauf-ID des RunStore bzw. c<CommentId> beim Backfill */
  id: string;
  kind: DispatchRunKind;
  /** Ticket- oder Fragen-ID (kein Fremdschlüssel) */
  subjectId: string;
  projectId: number;
  boardId: number | null;
  agent: string;
  attempt: number | null;
  profile: string;
  escalatedFrom: string | null;
  /** Modell-Alias aus Profil oder Label */
  requestedModel: string | null;
  /** Hauptmodell laut modelUsage */
  model: string | null;
  effort: string | null;
  startedAt: string;
  endedAt: string | null;
  durationMs: number | null;
  numTurns: number | null;
  exitCode: number | null;
  timedOut: boolean;
  outcome: DispatchRunOutcome;
  abortCategory: string | null;
  abortReason: string | null;
  costUsd: number | null;
  inputTokens: number | null;
  outputTokens: number | null;
  cacheReadTokens: number | null;
  cacheWriteTokens: number | null;
  /** Verbrauch je Modell als JSON-Text */
  modelUsage: string | null;
  source: DispatchRunSource;
  /** Schrittlimit, das für diesen Lauf galt */
  maxTurns: number | null;
  /** Zeitlimit in Minuten, das für diesen Lauf galt */
  timeoutMinutes: number | null;
  /** Warum das (Basis-)Profil gewählt wurde */
  profileReason: DispatchProfileReason | null;
  /** 1-basierte Nummer der Regel bei profileReason = 'rule' */
  ruleIndex: number | null;
  /** Version der Dispatcher-Einstellung (Hash) */
  configHash: string | null;
  /** Anzahl abgelehnter Werkzeugaufrufe */
  deniedCount: number | null;
  /** Abgelehnte Werkzeuge, kommagetrennt und eindeutig */
  deniedTools: string | null;
}

/** Eine Fassung der Dispatcher-Einstellung (Tabelle board.DispatchConfigVersions) */
export interface DispatchConfigVersion {
  /** Hash der wirksamen Einstellung (DispatchConfigFingerprint) */
  hash: string;
  /** Name des Dispatchers, der diese Fassung zuerst benutzt hat */
  dispatcher: string;
  /** Zeitpunkt des ersten Auftretens (ISO, UTC) */
  firstSeenAt: string;
  /** Die gehashten Teile der Einstellung als JSON-Text */
  content: string;
}

/** Läufe eines Projekts, deren Start in [from, to) liegt (ISO, UTC) */
export interface DispatchRunFilter {
  projectId: number;
  boardId?: number;
  from: string;
  to: string;
}

/** Zahl der [dispatch]-Kommentare „Start“ und „Abbruch“ eines Tickets */
export interface DispatchCommentCounts {
  starts: number;
  aborts: number;
}

/** Filter der Auswertung; from/to als Zeitpunkte (to ausschließlich) */
export interface DispatchStatsFilter {
  projectId: number;
  boardId?: number;
  from: Date;
  to: Date;
  /** Art der ausgewerteten Läufe (Standard work) */
  kind?: DispatchRunKind;
}

/** Ausgänge der Läufe eines Profils: gelöst, abgebrochen mit weiterem Versuch (eskaliert), endgültig gescheitert, Rest */
export interface DispatchProfileRow {
  profile: string;
  runs: number;
  done: number;
  escalated: number;
  final: number;
  other: number;
}

/** Ergebnis der Auswertung (Läufe einer Art, Start im Zeitraum) */
export interface DispatchStats {
  kind: DispatchRunKind;
  runsSince: string | null;
  backfilledRuns: number;
  /** Erstes Auftreten der neuesten Fassung der Dispatcher-Einstellung (ISO) */
  configChangedAt: string | null;
  kpis: {
    runs: number;
    /** Verschiedene Tickets bzw. Fragen */
    subjects: number;
    done: number;
    aborted: number;
    /** Gegenstände mit einem erfolgreichen Lauf */
    solved: number;
    /** Gelöste / abgeschlossene Gegenstände (gelöst oder endgültig gescheitert); wie die Erstversuch-Quote je Gegenstand */
    successRate: number | null;
    /** Abgeschlossene Gegenstände, die im ersten Lauf ohne Eskalation gelöst wurden */
    firstTryRate: number | null;
    /** Gegenstände mit mindestens einem Lauf nach einem Abbruch (Wiederholung oder Eskalation) */
    escalationRate: number | null;
    finalFailures: number;
  };
  /** Läufe im Zeitverlauf: bis 28 Tage je Tag, darüber je Woche (Montag); start = JJJJ-MM-TT in Ortszeit */
  series: { unit: 'day' | 'week'; points: Array<{ start: string; done: number; aborted: number; other: number }> };
  byProfile: DispatchProfileRow[];
  /** Abgeschlossene Gegenstände: im ersten Versuch gelöst, nach Eskalation gelöst, endgültig gescheitert (Anteile) */
  ladder: { finished: number; firstTry: number | null; afterEscalation: number | null; final: number | null };
  abortReasons: Array<{ category: string; label: string; count: number }>;
  /** Anteil der Abbrüche an Schritt-, Zeitlimit oder vollem Kontext (per Einstellung beeinflussbar) */
  configurableAbortShare: number | null;
  /** Pausen wegen Nutzungslimit im Zeitraum (alle Arten) */
  usagePauses: number;
  /** Läufe ohne Lebenszeichen (staleMinutes überschritten) im Zeitraum (alle Arten) */
  staleRuns: number;
}
