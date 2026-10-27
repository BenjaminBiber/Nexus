import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { InboxOrder } from '../../../src/board/inbox/InboxOrder.js';
import { InboxOrderFile } from '../../../src/board/inbox/InboxOrderFile.js';
import type { InboxItem } from '../../../src/board/inbox/InboxTypes.js';
import { HarnessHome } from '../../../src/registry/HarnessHome.js';

const item = (key: string): InboxItem => ({
  key, kind: 'blocked', title: key, detail: null, boardId: 1, boardName: 'B', projectKey: 'AIH', ticketId: null, questionId: null,
  noteId: null, waitingTickets: 0, priority: null, since: '', rank: 5,
});
const keys = (items: InboxItem[]) => items.map((i) => i.key);

describe('InboxOrder.apply', () => {
  const order = new InboxOrder();

  it('lässt die Reihenfolge ohne gespeicherte Schlüssel unverändert', () => {
    expect(keys(order.apply([item('ticket:A-1'), item('ticket:A-2')], []))).toEqual(['ticket:A-1', 'ticket:A-2']);
  });

  it('setzt neue Einträge oben in gegebener Reihenfolge, gespeicherte danach in Datei-Reihenfolge', () => {
    const items = [item('ticket:A-1'), item('note:1'), item('ticket:A-2'), item('question:A-F1')];
    expect(keys(order.apply(items, ['ticket:A-2', 'ticket:A-1']))).toEqual(['note:1', 'question:A-F1', 'ticket:A-2', 'ticket:A-1']);
  });

  it('übergeht Schlüssel ohne Eintrag und doppelte Schlüssel', () => {
    const items = [item('ticket:A-1'), item('ticket:A-2')];
    expect(keys(order.apply(items, ['ticket:A-9', 'ticket:A-2', 'ticket:A-1', 'ticket:A-2']))).toEqual(['ticket:A-2', 'ticket:A-1']);
  });
});

describe('InboxOrderFile', () => {
  let dir: string;
  let file: InboxOrderFile;

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'harness-inbox-'));
    file = new InboxOrderFile(new HarnessHome({ HARNESS_HOME: dir }));
  });
  afterEach(() => rmSync(dir, { recursive: true, force: true }));

  it('liest ohne Datei eine leere Reihenfolge', async () => {
    expect(await file.read()).toEqual([]);
  });

  it('schreibt inbox-order.json und liest sie wieder', async () => {
    await file.write(['question:AIH-F0001', 'ticket:AIH-0003', 'note:7']);
    expect(file.file).toBe(join(dir, 'inbox-order.json'));
    expect(JSON.parse(readFileSync(file.file, 'utf8'))).toEqual({ version: 1, keys: ['question:AIH-F0001', 'ticket:AIH-0003', 'note:7'] });
    expect(await file.read()).toEqual(['question:AIH-F0001', 'ticket:AIH-0003', 'note:7']);
  });

  it('liefert bei kaputter Datei eine leere Reihenfolge statt eines Fehlers', async () => {
    writeFileSync(file.file, '{ kaputt');
    expect(await file.read()).toEqual([]);
    writeFileSync(file.file, JSON.stringify({ version: 1, keys: 'x' }));
    expect(await file.read()).toEqual([]);
  });

  it('übergeht ungültige und doppelte Schlüssel', async () => {
    writeFileSync(file.file, JSON.stringify({ version: 1, keys: ['ticket:A-1', 42, 'quatsch', 'ticket:A-1', 'note:2'] }));
    expect(await file.read()).toEqual(['ticket:A-1', 'note:2']);
  });

  it('setzt mit clear zurück', async () => {
    await file.write(['ticket:A-1']);
    await file.clear();
    expect(await file.read()).toEqual([]);
    await file.clear();
  });
});
