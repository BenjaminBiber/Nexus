import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { writeFileSync } from 'node:fs';
import { afterEach, describe, expect, it } from 'vitest';
import { DISPATCH_MARKERS, REVIEW_REASON } from '@harness/core';
import { DispatchFixture, OK_OUTPUT } from '../../helpers/DispatchFixture.js';

// Zeile von stream-json: Werkzeugaufruf und sein Ergebnis
function testRun(id: string, command: string, output: string, isError: boolean): string {
  return [
    JSON.stringify({ type: 'assistant', message: { content: [{ type: 'tool_use', id, name: 'Bash', input: { command } }] } }),
    JSON.stringify({ type: 'user', message: { content: [{ type: 'tool_result', tool_use_id: id, content: output, is_error: isError }] } }),
  ].join('\n');
}

const RUN_OUTPUT = [testRun('t1', 'npx vitest run', 'Tests 3 passed', false), testRun('t2', 'npm run lint', '1 error', true), OK_OUTPUT].join('\n');

describe('DispatchService – Freigabe vor dem Merge', () => {
  let f: DispatchFixture;
  afterEach(() => f?.dispose());

  // Startet ein bereites Ticket, lässt den Agenten erledigen und wertet den Lauf aus
  async function toReview(title = 'Rabatt'): Promise<string> {
    const id = await f.ready(title);
    expect((await f.service.tick()).started).toEqual([id]);
    await f.finishAsAgent(id);
    await f.end(id, RUN_OUTPUT);
    const report = await f.service.tick();
    expect(report.evaluated).toEqual([{ ticketId: id, outcome: 'review' }]);
    expect(report.merged).toEqual([]);
    return id;
  }

  it('ein vom Lauf erledigtes Ticket wartet auf Freigabe statt zu mergen', async () => {
    f = await DispatchFixture.create({ reviewBeforeMerge: true });
    const id = await toReview();
    const ticket = await f.services.tickets.get(id);
    expect(ticket).toMatchObject({ status: 'blocked', blockedReason: REVIEW_REASON, assignee: null, branch: `ticket/${id}-rabatt` });
    expect(existsSync(join(f.repo.path, 'feature.txt'))).toBe(false);
    expect(existsSync(ticket.worktree!)).toBe(true);
    expect((await f.service.stores.state.read()).merges).toEqual({});
    const entry = await f.service.stores.reviews.read(id);
    expect(entry?.tests).toEqual([
      { command: 'npx vitest run', ok: true, output: 'Tests 3 passed' },
      { command: 'npm run lint', ok: false, output: '1 error' },
    ]);
    const review = (await f.comments(id)).find((c) => c.startsWith(DISPATCH_MARKERS.REVIEW + ':'));
    expect(review).toContain('1 Datei geändert (+1/−0)');
    expect(review).toContain('Tests: 1 grün, 1 rot');
    expect(f.notices.map((n) => n.kind)).toContain('review');
    // Ein weiterer Durchlauf ändert nichts
    const again = await f.service.tick();
    expect(again).toMatchObject({ started: [], merged: [], evaluated: [] });
    expect((await f.services.tickets.get(id)).status).toBe('blocked');
  });

  it('ohne Freigabe (reviewBeforeMerge: false) mergt derselbe Ablauf sofort', async () => {
    f = await DispatchFixture.create({ reviewBeforeMerge: false });
    const id = await f.ready('Sofort');
    await f.service.tick();
    await f.finishAsAgent(id);
    await f.end(id, RUN_OUTPUT);
    const report = await f.service.tick();
    expect(report.evaluated).toEqual([{ ticketId: id, outcome: 'done' }]);
    expect(report.merged).toEqual([id]);
    expect(await f.service.stores.reviews.read(id)).toBeNull();
  });

  it('„Übernehmen“ mergt wie bisher und löscht den Review-Eintrag', async () => {
    f = await DispatchFixture.create({ reviewBeforeMerge: true });
    const id = await toReview();
    const accepted = await f.service.accept(id, 'Florian');
    expect(accepted.result).toMatchObject({ outcome: 'merged' });
    const ticket = await f.services.tickets.get(id);
    expect(ticket).toMatchObject({ status: 'done', blockedReason: null, branch: null, worktree: null });
    expect(existsSync(join(f.repo.path, 'feature.txt'))).toBe(true);
    expect(ticket.history.some((h) => h.field === 'status' && h.to === 'done' && h.note === 'abgenommen' && h.byName === 'Florian')).toBe(true);
    expect(await f.service.stores.reviews.read(id)).toBeNull();
  });

  it('„Übernehmen“ mit Squash ergibt einen Commit', async () => {
    f = await DispatchFixture.create({ reviewBeforeMerge: true, mergeStrategy: 'squash' });
    const before = Number(f.repo.git('rev-list', '--count', 'HEAD'));
    const id = await toReview();
    expect((await f.service.accept(id, 'Florian')).result).toMatchObject({ outcome: 'merged' });
    expect(Number(f.repo.git('rev-list', '--count', 'HEAD'))).toBe(before + 1);
  });

  it('„Zurück an den Agenten“: Geplant im selben Worktree, Rückmeldung als Arbeitsanweisung, zählt nicht als Versuch', async () => {
    f = await DispatchFixture.create({ reviewBeforeMerge: true });
    const id = await toReview();
    const { worktree, branch } = await f.services.tickets.get(id);
    const returned = await f.service.returnToAgent(id, 'Bitte auch den Fehlerfall testen', 'Florian');
    expect(returned.result).toEqual({ outcome: 'returned', text: expect.stringContaining(id) });
    expect(await f.services.tickets.get(id)).toMatchObject({ status: 'planned', blockedReason: null, assignee: null, branch, worktree });
    const comment = (await f.comments(id)).at(-1)!;
    expect(comment).toBe(`${DISPATCH_MARKERS.REVIEW_RETURN} (von Florian): Bitte auch den Fehlerfall testen`);

    const report = await f.service.tick();
    expect(report.started).toEqual([id]);
    const record = await f.run(id);
    expect(record.cwd).toBe(worktree);
    expect(record.prompt).toContain('Rückmeldung aus dem Review');
    expect(record.prompt).toContain('Bitte auch den Fehlerfall testen');
    expect(record.run.profile).toBe('standard');
    const detail = await f.services.tickets.get(id);
    expect(detail.comments.filter((c) => c.body.startsWith(DISPATCH_MARKERS.ABORT))).toEqual([]);

    // Zweiter Lauf: wieder im Review, neue Tests
    await f.finishAsAgent(id, 'fehlerfall.txt', 'neu');
    await f.end(id, [testRun('t9', 'npm test', 'alles grün', false), OK_OUTPUT].join('\n'));
    expect((await f.service.tick()).evaluated).toEqual([{ ticketId: id, outcome: 'review' }]);
    expect((await f.service.stores.reviews.read(id))?.tests).toEqual([{ command: 'npm test', ok: true, output: 'alles grün' }]);
    // Im nächsten Start ist die alte Rückmeldung erledigt
    await f.service.returnToAgent(id, 'Noch der Name', 'Florian');
    await f.service.tick();
    const third = await f.run(id);
    expect(third.prompt).toContain('Noch der Name');
    expect(third.prompt).not.toContain('Bitte auch den Fehlerfall testen');
  });

  it('„Zurück an den Agenten“ lehnt leeren Text, Tickets ohne Review und laufende Tickets ab', async () => {
    f = await DispatchFixture.create({ reviewBeforeMerge: true });
    const id = await toReview();
    await expect(f.service.returnToAgent(id, '   ', 'Florian')).rejects.toMatchObject({ code: 'return-not-possible' });
    const plain = await f.ready('Normal');
    await expect(f.service.returnToAgent(plain, 'x', 'Florian')).rejects.toMatchObject({ code: 'return-not-possible' });
    await f.service.tick();
    await expect(f.service.returnToAgent(plain, 'x', 'Florian')).rejects.toMatchObject({ code: 'return-not-possible' });
  });

  it('Review-Ansicht: Dateien, Tests und Patch; unbekannte Pfade werden abgelehnt', async () => {
    f = await DispatchFixture.create({ reviewBeforeMerge: true });
    const id = await toReview();
    const view = await f.service.review(id);
    expect(view).toMatchObject({
      ticketId: id, branch: `ticket/${id}-rabatt`, available: true, reason: null,
      files: [expect.objectContaining({ path: 'feature.txt', status: 'A', additions: 1 })],
      tests: [expect.objectContaining({ ok: true }), expect.objectContaining({ ok: false })],
    });
    expect(view.runLog).toMatch(/\.log$/);
    expect((await f.service.reviewFile(id, 'feature.txt')).patch).toContain('+neu');
    await expect(f.service.reviewFile(id, '--output=x')).rejects.toMatchObject({ code: 'review-file-unknown' });
  });

  it('Review-Ansicht mit Zielbranch: Diff, Patch und Hinweis gegen den Zielbranch statt gegen den Hauptordner', async () => {
    f = await DispatchFixture.create({ reviewBeforeMerge: true });
    // release/1 hat einen Stand mehr als der ausgecheckte Branch; das Ticket zweigt beim Start von ihm ab
    const main = f.repo.git('rev-parse', '--abbrev-ref', 'HEAD');
    f.repo.git('checkout', '-q', '-b', 'release/1');
    writeFileSync(join(f.repo.path, 'release.txt'), 'release');
    f.repo.git('add', 'release.txt');
    f.repo.git('commit', '-q', '-m', 'Stand von release/1');
    f.repo.git('checkout', '-q', main);

    const id = await f.ready('Ziel', { targetBranch: 'release/1' });
    expect((await f.service.tick()).started).toEqual([id]);
    await f.finishAsAgent(id);
    await f.end(id, RUN_OUTPUT);
    expect((await f.service.tick()).evaluated).toEqual([{ ticketId: id, outcome: 'review' }]);

    // Kommentar beim Gate: nur die Änderung des Tickets (feature.txt), nicht release.txt
    const comment = (await f.comments(id)).find((c) => c.startsWith(DISPATCH_MARKERS.REVIEW + ':'));
    expect(comment).toContain('1 Datei geändert (+1/−0)');
    // Panel: Ziel ist der Zielbranch, Dateiliste ohne release.txt
    const view = await f.service.review(id);
    expect(view).toMatchObject({ available: true, target: 'release/1', files: [expect.objectContaining({ path: 'feature.txt', status: 'A' })] });
    expect(view.files).toHaveLength(1);
    expect(view.base).toBe(f.repo.git('rev-parse', 'release/1'));
    expect((await f.service.reviewFile(id, 'feature.txt')).patch).toContain('+neu');
    await expect(f.service.reviewFile(id, 'release.txt')).rejects.toMatchObject({ code: 'review-file-unknown' });
  });
});
