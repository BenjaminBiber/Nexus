import type { DispatchRunShotDto } from '../api/dispatchTypes.js';
import type { TranscriptEntry } from '../api/sessionDetailTypes.js';

/** Ein Tool-Aufruf im Verlauf */
export type ToolEntry = Extract<TranscriptEntry, { kind: 'tool' }>;

/**
 * Eine Zeile im Verlauf eines Laufs: ein einzelner Eintrag (mit seinen Screenshots) oder eine gefaltete Gruppe. Eine
 * Gruppe trägt alle Screenshots ihrer Aufrufe in der Reihenfolge der Ablage; die eines Eintrags sind die mit dessen
 * `id` als `toolUseId` (siehe `entryShots`).
 */
export type RunItem =
  | { type: 'entry'; key: string; entry: TranscriptEntry; shots: DispatchRunShotDto[] }
  | { type: 'group'; key: string; entries: TranscriptEntry[]; tools: number; label: string; open: boolean; shots: DispatchRunShotDto[] };

// Screenshots eines Eintrags aus einer Liste (z. B. denen seiner Gruppe), in deren Reihenfolge; kein Aufruf: keine
export function entryShots(entry: TranscriptEntry, shots: DispatchRunShotDto[]): DispatchRunShotDto[] {
  return entry.kind === 'tool' ? shots.filter((shot) => shot.toolUseId === entry.id) : [];
}
