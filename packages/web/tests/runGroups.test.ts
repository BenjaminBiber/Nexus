import { describe, expect, it } from 'vitest';
import type { DispatchRunShotDto } from '../src/api/dispatchTypes.js';
import type { TranscriptEntry } from '../src/api/sessionDetailTypes.js';
import { GROUP_MIN_TOOLS, groupRunEntries } from '../src/dbboard/runGroups.js';

const AT = '2026-10-16T08:16:00.000Z';
const ok = { text: 'ok', isError: false, truncated: false };
const tool = (id: string, name = 'Read', result: typeof ok | null = ok): TranscriptEntry => ({ kind: 'tool', id, at: AT, name, summary: id, input: null, result });
const text = (id: string): TranscriptEntry => ({ kind: 'text', id, at: AT, text: id });
const thinking = (id: string): TranscriptEntry => ({ kind: 'thinking', id, at: AT, text: id, durationMs: null });
const shot = (file: string, toolUseId: string): DispatchRunShotDto => ({ file, toolUseId, at: AT, mediaType: 'image/png', bytes: 1 });
// Kurzform einer Zeile für Erwartungen
const shape = (items: ReturnType<typeof groupRunEntries>) =>
  items.map((item) => (item.type === 'group' ? `group(${item.entries.map((entry) => entry.id).join(',')})` : item.entry.id));

describe('groupRunEntries', () => {
  it('faltet eine Folge ab drei unauffälligen Aufrufen', () => {
    const items = groupRunEntries([text('t1'), tool('a'), tool('b'), tool('c'), text('t2'), tool('d'), tool('e')], [], false);
    expect(GROUP_MIN_TOOLS).toBe(3);
    expect(shape(items)).toEqual(['t1', 'group(a,b,c)', 't2', 'd', 'e']);
    expect(items[1]).toMatchObject({ type: 'group', key: 'group:a', tools: 3, open: false, label: '3 Schritte · 3× Read' });
    expect(items[0]).toEqual({ type: 'entry', key: 't1', entry: text('t1'), shots: [] });
  });

  it('nimmt „Nachgedacht“ in die Gruppe, zählt es aber nicht als Schritt', () => {
    const items = groupRunEntries([thinking('n1'), tool('a'), thinking('n2'), tool('b'), tool('c')], [], false);
    expect(shape(items)).toEqual(['group(n1,a,n2,b,c)']);
    expect(items[0]).toMatchObject({ key: 'group:n1', tools: 3 });
    // Zwei Aufrufe reichen nicht, auch mit Nachdenken dazwischen
    expect(shape(groupRunEntries([tool('a'), thinking('n1'), tool('b')], [], false))).toEqual(['a', 'n1', 'b']);
  });

  it('lässt Fehler, offene Aufrufe und Aufrufe mit Screenshot einzeln stehen', () => {
    const fail = tool('f', 'Bash', { text: 'kaputt', isError: true, truncated: false });
    const open = tool('o', 'Bash', null);
    const entries = [tool('a'), tool('b'), tool('c'), fail, tool('d'), tool('s'), tool('e'), tool('g'), tool('h'), open];
    const shots = [shot('001.png', 's'), shot('002.png', 's'), shot('003.png', 'fremd')];
    const items = groupRunEntries(entries, shots, false);
    expect(shape(items)).toEqual(['group(a,b,c)', 'f', 'd', 's', 'group(e,g,h)', 'o']);
    expect(items[3]).toMatchObject({ type: 'entry', key: 's', shots: [shots[0], shots[1]] });
    expect(items[1]).toMatchObject({ type: 'entry', shots: [] });
  });

  it('lässt die Arbeitsanweisung am Anfang weg', () => {
    const prompt: TranscriptEntry = { kind: 'prompt', id: 'p', at: AT, text: 'Du bist junior-ARC-0001.' };
    expect(shape(groupRunEntries([prompt, text('t1')], [], false))).toEqual(['t1']);
    // Ein späterer Prompt (Rückfrage) bleibt stehen
    expect(shape(groupRunEntries([text('t1'), { ...prompt, id: 'p2' }], [], false))).toEqual(['t1', 'p2']);
  });

  it('beschriftet die Gruppe mit höchstens vier Tools in der Reihenfolge ihres Auftretens', () => {
    const entries = [tool('a', 'Read'), tool('b', 'Grep'), tool('c', 'Read'), tool('d', 'mcp__browser__snapshot'), tool('e', 'Bash'), tool('f', 'Glob'), tool('g', 'Read')];
    const [group] = groupRunEntries(entries, [], false);
    expect(group).toMatchObject({ label: '7 Schritte · 3× Read · 1× Grep · 1× browser · snapshot · 1× Bash · …' });
  });

  it('öffnet bei einem laufenden Lauf die letzte Gruppe, wenn dahinter nur offene Aufrufe stehen', () => {
    const entries = [tool('a'), tool('b'), tool('c'), text('t'), tool('d'), tool('e'), tool('f')];
    const open = (items: ReturnType<typeof groupRunEntries>) => items.map((item) => (item.type === 'group' ? item.open : null));
    // läuft, Gruppe am Ende
    expect(open(groupRunEntries(entries, [], true))).toEqual([false, null, true]);
    // läuft, dahinter ein Aufruf ohne Ergebnis
    expect(open(groupRunEntries([...entries, tool('o', 'Bash', null)], [], true))).toEqual([false, null, true, null]);
    // läuft, dahinter Text: der Agent ist weiter
    expect(open(groupRunEntries([...entries, text('t2')], [], true))).toEqual([false, null, false, null]);
    // beendet: alles zu
    expect(open(groupRunEntries(entries, [], false))).toEqual([false, null, false]);
  });

  it('liefert ohne Einträge nichts', () => {
    expect(groupRunEntries([], [], true)).toEqual([]);
  });
});
