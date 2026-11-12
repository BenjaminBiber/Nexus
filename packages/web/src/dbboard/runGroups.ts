import type { DispatchRunShotDto } from '../api/dispatchTypes.js';
import type { TranscriptEntry } from '../api/sessionDetailTypes.js';
import { toolDisplayName } from '../components/sessions/toolDisplay.js';
import type { RunItem } from './runItems.js';

/** Ab so vielen unauffälligen Tool-Aufrufen in Folge wird gefaltet */
export const GROUP_MIN_TOOLS = 3;
/** So viele Tools nennt die Beschriftung einer Gruppe höchstens */
const MAX_LABEL_TOOLS = 4;

// Screenshots je Tool-Aufruf, in der Reihenfolge der Ablage
function shotsByCall(shots: DispatchRunShotDto[]): Map<string, DispatchRunShotDto[]> {
  const byCall = new Map<string, DispatchRunShotDto[]>();
  for (const shot of shots) byCall.set(shot.toolUseId, [...(byCall.get(shot.toolUseId) ?? []), shot]);
  return byCall;
}

// Unauffällig: Nachgedacht sowie ein Aufruf mit Ergebnis, ohne Fehler und ohne Screenshot
function quiet(entry: TranscriptEntry, byCall: Map<string, DispatchRunShotDto[]>): boolean {
  if (entry.kind === 'thinking') return true;
  return entry.kind === 'tool' && entry.result !== null && !entry.result.isError && !byCall.has(entry.id);
}

// „7 Schritte · 4× Read · 2× Grep …“: Tools in der Reihenfolge ihres ersten Auftretens
function groupLabel(entries: TranscriptEntry[]): string {
  const counts = new Map<string, number>();
  for (const entry of entries) {
    if (entry.kind !== 'tool') continue;
    const name = toolDisplayName(entry.name);
    counts.set(name, (counts.get(name) ?? 0) + 1);
  }
  const tools = [...counts.values()].reduce((sum, count) => sum + count, 0);
  const parts = [...counts].slice(0, MAX_LABEL_TOOLS).map(([name, count]) => `${count}× ${name}`);
  if (counts.size > MAX_LABEL_TOOLS) parts.push('…');
  return [`${tools} Schritte`, ...parts].join(' · ');
}

// Bei einem laufenden Lauf bleibt die letzte Gruppe offen, wenn hinter ihr höchstens Aufrufe ohne Ergebnis stehen
function openLastGroup(items: RunItem[]): void {
  for (let i = items.length - 1; i >= 0; i -= 1) {
    const item = items[i]!;
    if (item.type === 'group') {
      items[i] = { ...item, open: true };
      return;
    }
    if (!(item.entry.kind === 'tool' && item.entry.result === null)) return;
  }
}

// Macht aus den Einträgen eines Laufs die Zeilen des Verlaufs: lange Folgen unauffälliger Schritte werden eine Gruppe,
// alles andere bleibt eine eigene Zeile und trägt die Screenshots seines Aufrufs
export function groupRunEntries(entries: TranscriptEntry[], shots: DispatchRunShotDto[], running: boolean): RunItem[] {
  const byCall = shotsByCall(shots);
  const list = entries[0]?.kind === 'prompt' ? entries.slice(1) : entries;
  const items: RunItem[] = [];
  let run: TranscriptEntry[] = [];

  const single = (entry: TranscriptEntry) => {
    items.push({ type: 'entry', key: entry.id, entry, shots: entry.kind === 'tool' ? (byCall.get(entry.id) ?? []) : [] });
  };
  // Schließt die laufende Folge ab: als Gruppe oder, wenn sie zu kurz ist, als einzelne Zeilen
  const close = () => {
    const tools = run.filter((entry) => entry.kind === 'tool').length;
    if (tools >= GROUP_MIN_TOOLS) items.push({ type: 'group', key: `group:${run[0]!.id}`, entries: run, tools, label: groupLabel(run), open: false });
    else run.forEach(single);
    run = [];
  };

  for (const entry of list) {
    if (quiet(entry, byCall)) {
      run.push(entry);
    } else {
      close();
      single(entry);
    }
  }
  close();
  if (running) openLastGroup(items);
  return items;
}
