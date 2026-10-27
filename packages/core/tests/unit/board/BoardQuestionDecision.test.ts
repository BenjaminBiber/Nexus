import { describe, expect, it } from 'vitest';
import { BoardFixture } from './helpers/BoardFixture.js';

// Ticket ARC-0001 mit Frage ARC-F0001 und zwei Recherchen (O1, O2 und O3); Version der Frage danach: 3
async function researched(): Promise<BoardFixture> {
  const f = await BoardFixture.create();
  await f.tickets.create(f.boardId, { title: 'Login' }, f.actor);
  await f.questions.create('ARC', { title: 'Welcher Speicher?', ticketIds: ['ARC-0001'] }, f.agent);
  await f.questions.addResearch('ARC-F0001', { options: [{ title: 'Datei', text: 'JSON' }, { title: 'DB', text: 'SQL', recommended: true }] }, f.agent);
  await f.questions.addResearch('ARC-F0001', { options: [{ title: 'Cache', text: 'RAM' }] }, f.agent);
  return f;
}

describe('BoardQuestionService – entscheiden', () => {
  it('beantwortet mit einer Option der neuesten Recherche und schreibt Verlauf an Frage und Ticket', async () => {
    const f = await researched();
    const q = await f.questions.answer('ARC-F0001', { option: 'O3', reason: 'schnell genug' }, f.human, { rev: 3 });
    expect(q).toMatchObject({
      state: 'answered', rev: 4, ready: false,
      decision: { outcome: 'answered', option: 'O3', text: null, reason: 'schnell genug', byName: 'Florian', byId: f.human.userId, at: '2026-10-13T10:00:00.000Z' },
    });
    expect(q.history.at(-1)).toMatchObject({ type: 'decided', text: 'Option O3 „Cache“ gewählt. Begründung: schnell genug', byKind: 'human' });
    expect((await f.tickets.get('ARC-0001')).history.at(-1)).toMatchObject({
      field: 'question', from: 'open', to: 'answered', note: 'Frage entschieden: ARC-F0001 „Welcher Speicher?“', byName: 'Florian',
    });
  });

  it('entscheidet ohne Begründung; der Verlauf nennt dann keine', async () => {
    const f = await researched();
    const answered = await f.questions.answer('ARC-F0001', { option: 'O3', reason: '  ' }, f.human);
    expect(answered.decision).toMatchObject({ option: 'O3', reason: '' });
    expect(answered.history.at(-1)?.text).toMatch(/^Option O3 .*gewählt.$/);
  });

  it('verlangt Mensch, offene Frage, eindeutige Wahl und eine Option der neuesten Recherche', async () => {
    const f = await researched();
    await expect(f.questions.answer('ARC-F0001', { option: 'O3', reason: 'x' }, f.agent)).rejects.toMatchObject({ code: 'human-only' });
    await expect(f.questions.answer('ARC-F0001', { option: 'O1', reason: 'x' }, f.human)).rejects.toMatchObject({ code: 'decision-option-unknown' });
    await expect(f.questions.answer('ARC-F0001', { option: 'O3', text: 'beides', reason: 'x' }, f.human)).rejects.toMatchObject({ code: 'decision-ambiguous' });
    await expect(f.questions.answer('ARC-F0001', { text: 'frei', reason: 'x' }, f.human, { rev: 1 })).rejects.toMatchObject({ code: 'conflict' });
    const answered = await f.questions.answer('ARC-F0001', { text: 'Erst CSV', reason: 'reicht' }, f.human);
    expect(answered.history.at(-1)?.text).toBe('Antwort: Erst CSV. Begründung: reicht');
    await expect(f.questions.answer('ARC-F0001', { text: 'nochmal', reason: 'x' }, f.human)).rejects.toMatchObject({ code: 'question-state-invalid' });
    expect((await f.questions.list('ARC', { state: 'answered' })).questions.map((q) => q.id)).toEqual(['ARC-F0001']);
  });

  it('verwirft mit Pflichtbegründung und öffnet wieder mit der früheren Entscheidung im Verlauf', async () => {
    const f = await researched();
    await expect(f.questions.discard('ARC-F0001', ' ', f.human)).rejects.toMatchObject({ code: 'decision-reason-missing' });
    const discarded = await f.questions.discard('ARC-F0001', 'doppelt', f.human);
    expect(discarded).toMatchObject({ state: 'discarded', decision: { outcome: 'discarded', reason: 'doppelt', option: null, text: null } });
    expect(discarded.history.at(-1)?.text).toBe('Verworfen. Begründung: doppelt');
    expect((await f.questions.list('ARC')).counts).toEqual({ open: 0, answered: 0, discarded: 1, mine: 0 });
    await expect(f.questions.reopen('ARC-F0001', undefined, f.agent)).rejects.toMatchObject({ code: 'human-only' });
    const reopened = await f.questions.reopen('ARC-F0001', 'neue Lage', f.human);
    expect(reopened).toMatchObject({ state: 'open', decision: null, ready: true });
    expect(reopened.history.at(-1)?.text).toBe('Wieder geöffnet. Grund: neue Lage. Frühere Entscheidung: Verworfen. Begründung: doppelt (Florian)');
    await expect(f.questions.reopen('ARC-F0001', undefined, f.human)).rejects.toMatchObject({ code: 'question-state-invalid' });
    expect((await f.tickets.get('ARC-0001')).history.slice(-2).map((h) => [h.from, h.to, h.note])).toEqual([
      ['open', 'discarded', 'Frage verworfen: ARC-F0001 „Welcher Speicher?“'],
      ['discarded', 'open', 'Frage wieder geöffnet: ARC-F0001 „Welcher Speicher?“'],
    ]);
  });

  it('kürzt den Verlaufseintrag am Ticket auf 200 Zeichen', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'Login' }, f.actor);
    await f.questions.create('ARC', { title: 'L'.repeat(300), ticketIds: ['ARC-0001'] }, f.agent);
    await f.questions.discard('ARC-F0001', 'zu lang', f.human);
    const note = (await f.tickets.get('ARC-0001')).history.at(-1)!.note!;
    expect(note).toHaveLength(200);
    expect(note.endsWith('…')).toBe(true);
  });
});
