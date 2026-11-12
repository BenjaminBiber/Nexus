import type { DispatchRunShotDto } from '../api/dispatchTypes.js';
import type { TranscriptEntry } from '../api/sessionDetailTypes.js';

/** Ein Tool-Aufruf im Verlauf */
export type ToolEntry = Extract<TranscriptEntry, { kind: 'tool' }>;

/** Eine Zeile im Verlauf eines Laufs: ein einzelner Eintrag (mit seinen Screenshots) oder eine gefaltete Gruppe */
export type RunItem =
  | { type: 'entry'; key: string; entry: TranscriptEntry; shots: DispatchRunShotDto[] }
  | { type: 'group'; key: string; entries: TranscriptEntry[]; tools: number; label: string; open: boolean };
