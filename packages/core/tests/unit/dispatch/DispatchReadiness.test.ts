import { describe, expect, it } from 'vitest';
import { DispatchConfigNormalizer } from '../../../src/dispatch/DispatchConfigNormalizer.js';
import { DispatchReadiness } from '../../../src/dispatch/DispatchReadiness.js';
import type { TicketView } from '../../../src/board/TicketService.js';

const ME = '11111111-1111-1111-1111-111111111111';
const normalizer = new DispatchConfigNormalizer();
const readiness = new DispatchReadiness(normalizer.normalize(normalizer.example()));
const board = { ownerId: ME, repoPath: 'C:/repo' };
const exists = () => true;

// Bereites Ticket; Felder lassen sich überschreiben
function ready(extra: Partial<TicketView> = {}): TicketView {
  return {
    id: 'ARC-0001', status: 'planned', assignee: null, needsPlanning: false, labels: ['auto'], plan: 'Plan', handbook: 'Hb', acceptance: 'Ok',
    priority: 'normal', dependsOpen: [], dependsUnmerged: [], waitingOnQuestions: [], ...extra,
  } as TicketView;
}

describe('DispatchReadiness', () => {
  it('nimmt ein vollständiges Ticket mit Label auto', () => {
    expect(readiness.reason(ready(), board, ME, exists)).toBeNull();
  });

  it('nennt den ersten fehlenden Punkt', () => {
    const cases: Array<[Partial<TicketView>, string]> = [
      [{ status: 'backlog' }, 'Status ist nicht Geplant'],
      [{ assignee: 'claude' }, 'bereits claude zugewiesen'],
      [{ needsPlanning: true }, 'Planungs-Flag ist noch gesetzt'],
      [{ labels: ['auto', 'manuell'] }, 'Label „manuell“'],
      [{ labels: [] }, 'Label „auto“ fehlt'],
      [{ plan: ' ' }, 'Kein Plan'],
      [{ handbook: '' }, 'Kein Handbuch'],
      [{ acceptance: '' }, 'Keine Abnahme'],
      [{ dependsOpen: ['ARC-0002'] }, 'Wartet auf ARC-0002'],
      [{ dependsUnmerged: ['ARC-0002'] }, 'wartet auf Merge von ARC-0002'],
      [{ dependsUnmerged: ['ARC-0002', 'ARC-0003'] }, 'wartet auf Merge von ARC-0002, ARC-0003'],
      [{ waitingOnQuestions: [{ id: 'ARC-F0001', title: 'x' }] }, 'Wartet auf Frage ARC-F0001'],
      [{ labels: ['auto', 'profil:gross'] }, 'Label profil:gross – dieses Profil gibt es nicht (klein, standard, schwer)'],
    ];
    for (const [extra, reason] of cases) expect(readiness.reason(ready(extra), board, ME, exists)).toBe(reason);
  });

  it('prüft Besitzer und Repo-Pfad des Boards', () => {
    expect(readiness.reason(ready(), { ...board, ownerId: 'x' }, ME, exists)).toBe('Board gehört nicht dir');
    expect(readiness.reason(ready(), { ...board, repoPath: null }, ME, exists)).toBe('Board hat keinen Repo-Pfad (harness board boards repo <id> <pfad>)');
    expect(readiness.reason(ready(), board, ME, () => false)).toBe('Repo-Pfad C:/repo gibt es nicht');
  });

  it('ohne Opt-in-Label zählt jedes Ticket', () => {
    const all = new DispatchReadiness(normalizer.normalize({ ...normalizer.example(), optInLabel: null }));
    expect(all.reason(ready({ labels: [] }), board, ME, exists)).toBeNull();
  });
});
