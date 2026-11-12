// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import type { DispatchRunLogDto } from '../src/api/dispatchTypes.js';
import { KIND_LABELS, RunAbortReason, RunHead, runOutcome } from '../src/components/db/run/RunHead.js';
import { formatDate } from '../src/dbboard/formatDate.js';
import { formatDuration } from '../src/sessions/format.js';

afterEach(cleanup);

const NAME = 'ARC-0001-20261016-101500.log';
const STARTED = '2026-10-16T08:15:00.000Z';
const run = (extra: Partial<DispatchRunLogDto> = {}): DispatchRunLogDto => ({ name: NAME, kind: 'work', started: STARTED, running: false, ...extra });

describe('runOutcome', () => {
  it.each([
    ['done', 'erledigt', 'ok'],
    ['review', 'Review', 'ok'],
    ['merge-session', 'Merge', 'ok'],
    ['research', 'Recherche', 'ok'],
    ['human', 'Abnahme', 'warn'],
    ['question', 'Frage', 'warn'],
    ['blocked', 'blockiert', 'warn'],
    ['pause-limit', 'Limit', 'warn'],
    ['taken', 'übernommen', 'warn'],
    ['gone', 'Ticket weg', 'warn'],
    ['abort', 'Abbruch', 'error'],
    ['abort-final', 'Abbruch', 'error'],
    ['setup-failed', 'Einrichtung gescheitert', 'error'],
    ['pause-claude-missing', 'Claude fehlt', 'error'],
  ])('nennt das Ergebnis %s „%s“ im Ton %s', (outcome, label, tone) => {
    expect(runOutcome(run({ outcome }))).toEqual({ label, tone });
  });

  it('nennt einen laufenden Lauf „läuft“, was auch immer aufgezeichnet ist', () => {
    expect(runOutcome(run({ running: true }))).toEqual({ label: 'läuft', tone: 'running' });
    expect(runOutcome(run({ running: true, outcome: 'abort' }))).toEqual({ label: 'läuft', tone: 'running' });
  });

  it('nennt einen Lauf ohne oder mit unbekanntem Ergebnis „beendet“', () => {
    expect(runOutcome(run())).toEqual({ label: 'beendet', tone: 'neutral' });
    expect(runOutcome(run({ outcome: null }))).toEqual({ label: 'beendet', tone: 'neutral' });
    expect(runOutcome(run({ outcome: 'etwas-neues' }))).toEqual({ label: 'beendet', tone: 'neutral' });
  });
});

describe('RunHead', () => {
  it('zeigt Marke, Titel, Dauer und Modell', () => {
    render(<RunHead run={run({ outcome: 'done', durationMs: 720_000, model: 'claude-opus-5-5' })} />);
    expect(screen.getByText('erledigt').className).toBe('badge outcome-ok');
    expect(screen.getByText(`Lauf vom ${formatDate(STARTED)}`)).toBeTruthy();
    expect(screen.getByText(`${formatDuration(720_000)} · opus-5-5`)).toBeTruthy();
  });

  it('zeigt einen laufenden Lauf mit der Marke „läuft“ und ohne Dauer', () => {
    const { container } = render(<RunHead run={run({ running: true })} />);
    expect(screen.getByText('läuft').className).toBe('badge running');
    expect(container.querySelector('.run-facts')).toBeNull();
  });

  it('färbt Abbruch und Frage und lässt Fehlendes weg', () => {
    const { container, rerender } = render(<RunHead run={run({ outcome: 'abort', durationMs: 60_000 })} />);
    expect(screen.getByText('Abbruch').className).toBe('badge outcome-error');
    expect(container.querySelector('.run-facts')!.textContent).toBe(formatDuration(60_000));
    rerender(<RunHead run={run({ outcome: 'question', model: 'fake-opus' })} />);
    expect(screen.getByText('Frage').className).toBe('badge outcome-warn');
    expect(container.querySelector('.run-facts')!.textContent).toBe('fake-opus');
    rerender(<RunHead run={run()} />);
    expect(screen.getByText('beendet').className).toBe('badge outcome-neutral');
  });

  it('nennt die Art des Laufs und ohne Startzeit den Namen der Datei', () => {
    expect(KIND_LABELS).toEqual({ work: 'Lauf', merge: 'Merge-Agent', research: 'Recherche', plan: 'Planung' });
    const { rerender } = render(<RunHead run={run({ kind: 'merge' })} />);
    expect(screen.getByText(`Merge-Agent vom ${formatDate(STARTED)}`)).toBeTruthy();
    rerender(<RunHead run={run({ kind: 'plan', started: null })} />);
    expect(screen.getByText(`Planung vom ${NAME}`)).toBeTruthy();
  });
});

describe('RunAbortReason', () => {
  it('zeigt den Abbruchgrund eines beendeten Laufs', () => {
    render(<RunAbortReason run={run({ outcome: 'abort', abortReason: 'Zeitlimit nach 60 min' })} />);
    expect(screen.getByText('Zeitlimit nach 60 min').className).toBe('run-abort-reason');
  });

  it('zeigt nichts ohne Grund oder solange der Lauf läuft', () => {
    const { container, rerender } = render(<RunAbortReason run={run({ outcome: 'done' })} />);
    expect(container.innerHTML).toBe('');
    rerender(<RunAbortReason run={run({ running: true, abortReason: 'alter Grund' })} />);
    expect(container.innerHTML).toBe('');
  });
});
