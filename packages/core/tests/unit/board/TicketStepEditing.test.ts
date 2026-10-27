import { describe, expect, it } from 'vitest';
import { BoardFixture } from './helpers/BoardFixture.js';

const STEPS = [{ text: 'Test schreiben', done: false, detail: 'Erwartet: rot' }, { text: 'Umsetzen', done: false }];

describe('Schritte anlegen, bearbeiten, löschen und verschieben', () => {
  it('hängt Schritte an, auch an einem Ticket ohne Schritte, und fügt an Position ein', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
    const first = await f.tickets.addStep('ARC-0001', '  Test schreiben ', f.actor, { detail: 'Erwartet: rot' });
    expect(first).toMatchObject({ rev: 2, stepsTotal: 1, steps: [{ text: 'Test schreiben', done: false, detail: 'Erwartet: rot' }] });
    expect(first.history.at(-1)).toMatchObject({ field: 'step', from: null, to: '1: Test schreiben', note: 'Schritt hinzugefügt' });
    await f.tickets.addStep('arc-1', 'Committen', f.actor);
    const inserted = await f.tickets.addStep('ARC-0001', 'Umsetzen', f.actor, { at: 2 });
    expect(inserted.steps.map((s) => s.text)).toEqual(['Test schreiben', 'Umsetzen', 'Committen']);
    expect(inserted.rev).toBe(4);
    await expect(f.tickets.addStep('ARC-0001', 'X', f.actor, { at: 5 })).rejects.toMatchObject({ code: 'step-not-found', message: expect.stringContaining('1 bis 4') });
    await expect(f.tickets.addStep('ARC-0001', '  ', f.actor)).rejects.toMatchObject({ code: 'step-empty' });
    await expect(f.tickets.addStep('ARC-0001', 'x'.repeat(1001), f.actor)).rejects.toMatchObject({ code: 'value-too-long' });
    await expect(f.tickets.addStep('ARC-0099', 'X', f.actor)).rejects.toMatchObject({ code: 'ticket-not-found' });
  });

  it('bearbeitet Text und Anleitung, behält den Haken und entfernt eine leere Anleitung', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A', steps: STEPS }, f.actor);
    await f.tickets.setStep('ARC-0001', 1, true, f.actor);
    const edited = await f.tickets.editStep('ARC-0001', 1, { text: 'Test zuerst' }, f.actor);
    expect(edited.steps[0]).toEqual({ text: 'Test zuerst', done: true, detail: 'Erwartet: rot' });
    expect(edited.history.at(-1)).toMatchObject({ field: 'step', from: '1: Test schreiben', to: '1: Test zuerst', note: 'Schritt bearbeitet' });
    const detail = await f.tickets.editStep('ARC-0001', 2, { detail: 'npm test' }, f.actor);
    expect(detail.steps[1]).toEqual({ text: 'Umsetzen', done: false, detail: 'npm test' });
    expect(detail.history.at(-1)).toMatchObject({ note: 'Anleitung geändert' });
    const cleared = await f.tickets.editStep('ARC-0001', 1, { detail: '' }, f.actor);
    expect(cleared.steps[0]).toEqual({ text: 'Test zuerst', done: true });
    const same = await f.tickets.editStep('ARC-0001', 1, { text: 'Test zuerst' }, f.actor);
    expect(same.rev).toBe(cleared.rev);
    await expect(f.tickets.editStep('ARC-0001', 3, { text: 'X' }, f.actor)).rejects.toMatchObject({ code: 'step-not-found' });
    await expect(f.tickets.editStep('ARC-0001', 1, {}, f.actor)).rejects.toMatchObject({ code: 'nothing-changed' });
    await expect(f.tickets.editStep('ARC-0001', 1, { text: ' ' }, f.actor)).rejects.toMatchObject({ code: 'step-empty' });
  });

  it('löscht Schritte mit Verlauf; die folgenden rücken nach', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A', steps: STEPS }, f.actor);
    const removed = await f.tickets.removeStep('ARC-0001', 1, f.actor);
    expect(removed.steps).toEqual([{ text: 'Umsetzen', done: false }]);
    expect(removed).toMatchObject({ rev: 2, stepsTotal: 1 });
    expect(removed.history.at(-1)).toMatchObject({ field: 'step', from: '1: Test schreiben', to: null, note: 'Schritt gelöscht' });
    await f.tickets.removeStep('ARC-0001', 1, f.actor);
    await expect(f.tickets.removeStep('ARC-0001', 1, f.actor)).rejects.toMatchObject({ code: 'step-not-found', message: expect.stringContaining('keine Schritte') });
  });

  it('verschiebt Schritte samt Haken und Anleitung', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A', steps: [...STEPS, { text: 'Committen', done: true }] }, f.actor);
    const moved = await f.tickets.moveStep('ARC-0001', 3, 1, f.actor);
    expect(moved.steps.map((s) => s.text)).toEqual(['Committen', 'Test schreiben', 'Umsetzen']);
    expect(moved.steps[0].done).toBe(true);
    expect(moved.history.at(-1)).toMatchObject({ field: 'step', from: '3: Committen', to: '1: Committen', note: 'Schritt verschoben' });
    const down = await f.tickets.moveStep('ARC-0001', 2, 3, f.actor);
    expect(down.steps.map((s) => s.text)).toEqual(['Committen', 'Umsetzen', 'Test schreiben']);
    expect(down.steps[2].detail).toBe('Erwartet: rot');
    const same = await f.tickets.moveStep('ARC-0001', 2, 2, f.actor);
    expect(same.rev).toBe(down.rev);
    await expect(f.tickets.moveStep('ARC-0001', 1, 4, f.actor)).rejects.toMatchObject({ code: 'step-not-found' });
    await expect(f.tickets.moveStep('ARC-0001', 0, 1, f.actor)).rejects.toMatchObject({ code: 'step-not-found' });
  });

  it('prüft erwarteten Text und Version (Schutz bei gleichzeitigen Änderungen)', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A', steps: STEPS }, f.actor);
    // Ein anderer verschiebt Schritt 2 nach oben; wer noch „1 = Test schreiben“ kennt, trifft daneben
    await f.tickets.moveStep('ARC-0001', 2, 1, f.actor);
    const conflict = { code: 'conflict', message: 'ARC-0001: Schritt 1 ist inzwischen „Umsetzen“ – die Schritte wurden geändert.' };
    await expect(f.tickets.editStep('ARC-0001', 1, { text: 'X' }, f.actor, { expectText: 'Test schreiben' })).rejects.toMatchObject(conflict);
    await expect(f.tickets.removeStep('ARC-0001', 1, f.actor, { expectText: 'Test schreiben' })).rejects.toMatchObject(conflict);
    await expect(f.tickets.moveStep('ARC-0001', 1, 2, f.actor, { expectText: 'Test schreiben' })).rejects.toMatchObject(conflict);
    await expect(f.tickets.setStep('ARC-0001', 1, true, f.actor, { expectText: 'Test schreiben' })).rejects.toMatchObject(conflict);
    await expect(f.tickets.addStep('ARC-0001', 'Neu', f.actor, { at: 1, expectText: 'Test schreiben' })).rejects.toMatchObject(conflict);
    await expect(f.tickets.removeStep('ARC-0001', 3, f.actor, { expectText: 'Weg' })).rejects.toMatchObject({ code: 'conflict', message: expect.stringContaining('gibt es inzwischen nicht mehr') });
    const unchanged = await f.tickets.get('ARC-0001');
    expect(unchanged.steps.map((s) => s.text)).toEqual(['Umsetzen', 'Test schreiben']);
    // Passender Text (Rand-Leerraum egal) und passende Version gehen durch
    const removed = await f.tickets.removeStep('ARC-0001', 2, f.actor, { expectText: ' Test schreiben ', rev: unchanged.rev });
    expect(removed.steps.map((s) => s.text)).toEqual(['Umsetzen']);
    await expect(f.tickets.setStep('ARC-0001', 1, true, f.actor, { rev: unchanged.rev })).rejects.toMatchObject({ code: 'conflict' });
    await expect(f.tickets.addStep('ARC-0001', 'Neu', f.actor, { rev: unchanged.rev })).rejects.toMatchObject({ code: 'conflict' });
    const done = await f.tickets.setStep('ARC-0001', 1, true, f.actor, { rev: removed.rev, expectText: 'Umsetzen' });
    expect(done.steps[0].done).toBe(true);
  });
});
