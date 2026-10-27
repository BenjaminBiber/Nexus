import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { TempRepo } from '../helpers/TempRepo.js';
import { HarnessProject } from '../../../src/project/HarnessProject.js';
import { ContextBuilder } from '../../../src/hooks/ContextBuilder.js';
import { QuestionContextMarks } from '../../../src/hooks/QuestionContextMarks.js';
import { BoardServices } from '../../../src/board/BoardServices.js';
import { WORKFLOW_PHASES } from '../../../src/workflow/WorkflowPhases.js';
import { BoardFixture } from '../board/helpers/BoardFixture.js';

const HUMAN = { userId: '11111111-1111-1111-1111-111111111111', name: 'Florian' };

describe('ContextBuilder (DB-Board)', () => {
  let repo: TempRepo;
  let f: BoardFixture;
  let builder: ContextBuilder;
  // Baut den Kontext und markiert die Kommentare wie der Hook nach der Ausgabe als gelesen
  const text = async (...args: Parameters<ContextBuilder['build']>) => {
    const context = await builder.build(...args);
    await builder.markRead(context.commentIds);
    return context.text;
  };

  beforeEach(async () => {
    repo = new TempRepo();
    const project = await HarnessProject.init(repo.path);
    f = await BoardFixture.create();
    const marks = new QuestionContextMarks(project.layout, () => f.now.getTime());
    builder = new ContextBuilder(new BoardServices(f.db, f.clock, f.repoPaths), marks, f.clock);
    await f.tickets.create(f.boardId, { title: 'Login', spec: '001-x', specTask: 'T001', steps: [{ text: 'A', done: false }] }, f.actor);
    await f.tickets.create(f.boardId, { title: 'Zweites' }, f.actor);
  });
  afterEach(() => repo.dispose());

  it('nennt zum Sessionbeginn Ticket, Spec, Schritte, Regeln mit harness board und den Ablauf', async () => {
    const out = (await text('session-start', ['ARC-0001']))!;
    expect(out).toContain('ARC-0001 „Login“ (Spec 001-x, Schritte 0/1)');
    for (const part of ['harness board comment ARC-0001', 'harness board show ARC-0001', 'harness board step ARC-0001', 'harness board done ARC-0001', 'Ablauf im Harness']) {
      expect(out, part).toContain(part);
    }
    for (const phase of WORKFLOW_PHASES) for (const step of phase.steps) if (step.command) expect(out).toContain(step.command);
    expect(out).toContain('Stand: ARC-0001 ist Backlog. Vorschlag: vor dem Umsetzen planen: Plan, Handbuch und Diagramm mit `harness board plan-json ARC-0001 --file payload.json --by claude` schreiben (setzt Geplant); erst danach mit /ai-harness:harness-implement ARC-0001 umsetzen.');
    expect(out).toContain('Aus einem Superpowers-Plan legst du auf Wunsch je Task ein Ticket an');
    expect(out).toContain('Tickets mit Planungsbedarf (etwa aus der Ticket-Auswahl der Docs-Seite) werden vor dem Umsetzen geplant: `harness --json board next-planning --claim claude` holt das nächste');
    expect(out).not.toContain('/speckit-');
  });

  it('schlägt für Tickets in Planung vor, den Plan mit dem Nutzer abzuschließen, und für geplante das Umsetzen', async () => {
    await f.tickets.update('ARC-0001', { status: 'in_planning' }, f.actor);
    const planning = (await text('session-start', ['ARC-0001']))!;
    expect(planning).toContain('Stand: ARC-0001 ist In Planung. Vorschlag: den Plan mit dem Nutzer abschließen');
    expect(planning).toContain('harness board plan-json ARC-0001');
    await f.tickets.update('ARC-0001', { status: 'planned' }, f.actor);
    expect((await text('session-start', ['ARC-0001']))!).toContain('Vorschlag: mit /ai-harness:harness-implement ARC-0001 umsetzen.');
  });

  it('weist ohne Ticket auf claim hin', async () => {
    const out = (await text('session-start', []))!;
    expect(out).toContain('harness board claim');
    expect(out).toContain('Stand: keinem Ticket zugeordnet');
  });

  it('schlägt je nach Stand den nächsten Schritt vor', async () => {
    await f.tickets.update('ARC-0001', { status: 'in_progress' }, f.actor);
    expect((await text('session-start', ['ARC-0001']))!).toContain('Stand: ARC-0001 ist In Arbeit. Vorschlag: nach der Umsetzung Review und Abschluss mit `harness board done` vorschlagen.');
    await f.tickets.move('ARC-0001', 'done', f.actor, { acceptance: 'ok' });
    const done = (await text('session-start', ['ARC-0001']))!;
    expect(done).toContain('Stand: ARC-0001 ist erledigt.');
    expect(done).toContain('harness board list --status geplant --md');
  });

  it('speist ungelesene Kommentare von Menschen genau einmal ein; eigene Kommentare der Session nie', async () => {
    await f.comments.add('ARC-0001', 'Bitte prüfen', HUMAN);
    await f.comments.add('ARC-0001', 'Eigener Stand', f.actor, { readByAgent: true });
    const first = (await text('user-prompt', ['ARC-0001']))!;
    expect(first).toContain('Neue Kommentare zu ARC-0001:');
    expect(first).toContain('Florian: Bitte prüfen');
    expect(first).not.toContain('Eigener Stand');
    expect(first).not.toContain('Ablauf im Harness');
    expect(await text('user-prompt', ['ARC-0001'])).toBeNull();
    await f.comments.add('ARC-0002', 'Gesammelt', HUMAN);
    expect((await text('session-start', ['ARC-0002']))!).toContain('Gesammelt');
  });

  it('nennt offene Fragen zu den Tickets beim Start und beim Prompt nur neue oder geänderte', async () => {
    const q = await f.questions.create('ARC', { title: 'Welcher Speicher?', ticketIds: ['ARC-0001'] }, f.agent);
    await f.questions.create('ARC', { title: 'Andere Frage', ticketIds: ['ARC-0002'] }, f.agent);
    const start = (await text('session-start', ['ARC-0001'], 's-1'))!;
    expect(start).toContain('Offene Fragen (1, nicht entscheiden):');
    expect(start).toContain(`- ${q.id} Welcher Speicher? (betrifft ARC-0001)`);
    expect(start).toContain(`harness board question research ${q.id} --from <datei> --kind session`);
    expect(start).not.toContain('Andere Frage');
    expect(await text('user-prompt', ['ARC-0001'], 's-1')).toBeNull();
    f.advance(1);
    await f.questions.update(q.id, { title: 'Welcher Speicher genau?' }, f.human);
    expect((await text('user-prompt', ['ARC-0001'], 's-1'))!).toContain('Welcher Speicher genau?');
    expect(await text('user-prompt', ['ARC-0001'], 's-1')).toBeNull();
  });

  it('ohne Session-Kennung nennt der Prompt alle offenen Fragen', async () => {
    await f.questions.create('ARC', { title: 'Offen?', ticketIds: ['ARC-0001'] }, f.agent);
    expect((await text('user-prompt', ['ARC-0001']))!).toContain('Offen?');
  });

  it('übergeht ein unbekanntes Ticket, ohne zu scheitern', async () => {
    const out = (await text('session-start', ['ARC-0099', 'ARC-0001']))!;
    expect(out).toContain('ARC-0001');
    expect(out).not.toContain('ARC-0099');
  });

  it('markiert Kommentare erst mit markRead als gelesen; ohne markRead kommen sie beim nächsten Mal wieder', async () => {
    await f.comments.add('ARC-0001', 'Bitte prüfen', HUMAN);
    const first = await builder.build('user-prompt', ['ARC-0001']);
    expect(first.text).toContain('Bitte prüfen');
    expect(first.commentIds).toHaveLength(1);
    const again = await builder.build('user-prompt', ['ARC-0001']);
    expect(again.text).toContain('Bitte prüfen');
    await builder.markRead(again.commentIds);
    expect(await builder.build('user-prompt', ['ARC-0001'])).toEqual({ text: null, commentIds: [] });
  });

  it('nennt mehrere ungelesene Kommentare eines Tickets in zeitlicher Reihenfolge', async () => {
    await f.comments.add('ARC-0001', 'Erster', HUMAN);
    f.advance(1);
    await f.comments.add('ARC-0001', 'Zweiter', HUMAN);
    f.advance(1);
    await f.comments.add('ARC-0001', 'Dritter', HUMAN);
    const out = (await text('user-prompt', ['ARC-0001']))!;
    expect(out.match(/Neue Kommentare zu ARC-0001:/g)).toHaveLength(1);
    const order = ['Erster', 'Zweiter', 'Dritter'].map((body) => out.indexOf(`Florian: ${body}`));
    expect(order.every((i) => i >= 0)).toBe(true);
    expect(order).toEqual([...order].sort((a, b) => a - b));
  });

  it('scheitert bei anderen Fehlern als einem fehlenden Ticket, statt „keinem Ticket zugeordnet“ zu melden', async () => {
    const services = new BoardServices(f.db, f.clock, f.repoPaths);
    const tickets = { get: async () => Promise.reject(new Error('Verbindung unterbrochen')) } as unknown as BoardServices['tickets'];
    const failing = new ContextBuilder({ tickets, comments: services.comments, db: services.db }, null, f.clock);
    await expect(failing.build('session-start', ['ARC-0001'])).rejects.toThrow('Verbindung unterbrochen');
  });
  it('erlaubt schnelle Aufgaben ohne Ablauf nur nach ausdrücklicher Bestätigung', async () => {
    for (const ids of [[], ['ARC-0001']]) {
      const out = (await text('session-start', ids))!;
      expect(out).toContain('Schnelle Aufgaben und Nebenaufgaben');
      expect(out).toContain('ohne den Ablauf');
      expect(out).toContain('ausdrücklich');
      expect(out).toContain('„Ohne Workflow direkt umsetzen?“');
    }
  });

  it('formuliert den Vorschlag ohne Befehl, wenn planen und umsetzen keinen Schritt mit Befehl haben', async () => {
    const phases = WORKFLOW_PHASES.map((p) => (p.id === 'planen' || p.id === 'umsetzen' ? { ...p, steps: [] } : p));
    const hint = new ContextBuilder(new BoardServices(f.db, f.clock, f.repoPaths), null, f.clock, phases);
    // Kein Ticket, offenes Ticket (geplant) und erledigtes Ticket
    const none = (await hint.build('session-start', [])).text!;
    await f.tickets.update('ARC-0002', { status: 'planned' }, f.actor);
    const open = (await hint.build('session-start', ['ARC-0002'])).text!;
    await f.tickets.move('ARC-0002', 'done', f.actor, { force: true, acceptance: 'geprüft' });
    const done = (await hint.build('session-start', ['ARC-0002'])).text!;
    expect(none).toContain('eine neue Funktion planen (siehe Phase „Neue Funktion planen“)');
    expect(open).toContain('ARC-0002 umsetzen (siehe Phase „Ticket umsetzen“)');
    expect(done).toContain('und das Ticket umsetzen (siehe Phase „Ticket umsetzen“)');
    for (const out of [none, open, done]) {
      // Kein leerer Befehl: weder doppelte Leerzeichen noch ein Doppelpunkt ohne Befehle dahinter
      expect(out).not.toMatch(/ {2}/);
      expect(out).not.toMatch(/: [;.]/);
      expect(out).not.toContain('mit  ');
    }
  });

  it('nennt die Befehle des angewendeten Workflows', async () => {
    const phases = WORKFLOW_PHASES.map((p) => (p.id === 'umsetzen' ? { ...p, steps: [...p.steps, { step: 'Review', command: '/ai-harness:mein-review', when: 'x' }] } : p));
    const out = (await new ContextBuilder(new BoardServices(f.db, f.clock, f.repoPaths), null, f.clock, phases).build('session-start', [])).text!;
    expect(out).toContain('/ai-harness:mein-review');
  });
});
