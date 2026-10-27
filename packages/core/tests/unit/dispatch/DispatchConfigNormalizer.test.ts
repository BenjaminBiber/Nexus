import { describe, expect, it } from 'vitest';
import { HarnessError } from '../../../src/errors/HarnessError.js';
import { DispatchConfigNormalizer } from '../../../src/dispatch/DispatchConfigNormalizer.js';
import { DEFAULT_ALLOWED_TOOLS } from '../../../src/dispatch/DispatchConfig.js';

const normalizer = new DispatchConfigNormalizer();

// Erwartet einen Prüffehler mit passendem Text
function rejects(raw: unknown, text: RegExp): void {
  try {
    normalizer.normalize(raw as never);
  } catch (error) {
    expect(error).toBeInstanceOf(HarnessError);
    expect((error as HarnessError).code).toBe('dispatch-config-invalid');
    expect((error as Error).message).toMatch(text);
    return;
  }
  throw new Error('kein Fehler');
}

describe('DispatchConfigNormalizer', () => {
  it('füllt leere Einstellung mit Vorgaben', () => {
    const cfg = normalizer.normalize({});
    expect(cfg).toMatchObject({
      enabled: false, boards: null, wip: 1, window: null, optInLabel: 'auto', optOutLabel: 'manuell', agentPrefix: 'junior-',
      dispatcherName: 'dispatcher', maxAttempts: 2, pauseMinutesOnLimit: 60, promptExtra: '', escalate: true, allowFable: false,
      defaultProfile: 'standard',
    });
    expect(cfg.claude).toMatchObject({ command: 'claude', model: 'sonnet', effort: null, maxTurns: 80, timeoutMinutes: 60, permissionMode: 'acceptEdits', useSubscription: true, extraArgs: [] });
    expect(cfg.claude.allowedTools).toEqual(DEFAULT_ALLOWED_TOOLS);
    expect(cfg.profiles).toEqual({ standard: { model: 'sonnet', effort: null, maxTurns: 80, timeoutMinutes: 60 } });
    expect(cfg.ladder).toEqual(['standard']);
    expect(cfg.staleMinutes).toBe(90);
  });

  it('übernimmt Profile, Regeln, Leiter und Boards', () => {
    const cfg = normalizer.normalize(normalizer.example());
    expect(Object.keys(cfg.profiles)).toEqual(['klein', 'standard', 'schwer']);
    expect(cfg.profiles.schwer).toEqual({ model: 'opus', effort: 'max', maxTurns: 150, timeoutMinutes: 120 });
    expect(cfg.rules[0]).toEqual({ when: { labels: ['doku', 'tippfehler', 'klein'] }, profile: 'klein' });
    expect(cfg.ladder).toEqual(['klein', 'standard', 'schwer']);
    expect(cfg.staleMinutes).toBe(150);
    expect(cfg.enabled).toBe(false);
    expect(normalizer.normalize({ boards: ['3', 5] }).boards).toEqual([3, 5]);
    expect(normalizer.normalize({ boards: [] }).boards).toBeNull();
    expect(normalizer.normalize({ escalation: ['standard'] }).ladder).toEqual(['standard']);
  });

  it('nimmt staleMinutes nur über dem längsten Zeitlimit', () => {
    expect(normalizer.normalize({ staleMinutes: 65 }).staleMinutes).toBe(90);
    expect(normalizer.normalize({ staleMinutes: 200 }).staleMinutes).toBe(200);
  });

  it('leeres Opt-in-Label heißt alle Tickets', () => {
    expect(normalizer.normalize({ optInLabel: '' }).optInLabel).toBeNull();
    expect(normalizer.normalize({ optInLabel: null }).optInLabel).toBeNull();
    expect(normalizer.normalize({ optOutLabel: null }).optOutLabel).toBeNull();
  });

  it('übergeht die frühere Einstellung mergeIgnore', () => {
    expect(normalizer.normalize({ mergeIgnore: ['*.yaml'] } as never)).not.toHaveProperty('mergeIgnore');
  });

  it('Merge-Strategie: Vorgabe merge, squash wird übernommen, Beispiel nennt sie', () => {
    expect(normalizer.normalize({}).mergeStrategy).toBe('merge');
    expect(normalizer.normalize({ mergeStrategy: null as never }).mergeStrategy).toBe('merge');
    expect(normalizer.normalize({ mergeStrategy: 'squash' }).mergeStrategy).toBe('squash');
    expect(normalizer.normalize({ mergeStrategy: ' squash ' as never }).mergeStrategy).toBe('squash');
    expect(normalizer.example().mergeStrategy).toBe('merge');
    rejects({ mergeStrategy: 'rebase' as never }, /mergeStrategy/);
    rejects({ mergeStrategy: 1 as never }, /mergeStrategy/);
  });

  it('Freigabe vor dem Merge: Vorgabe true, false wird übernommen, Nicht-Boolean ist ein Fehler, Beispiel nennt sie', () => {
    expect(normalizer.normalize({}).reviewBeforeMerge).toBe(true);
    expect(normalizer.normalize({ reviewBeforeMerge: null as never }).reviewBeforeMerge).toBe(true);
    expect(normalizer.normalize({ reviewBeforeMerge: true }).reviewBeforeMerge).toBe(true);
    expect(normalizer.normalize({ reviewBeforeMerge: false }).reviewBeforeMerge).toBe(false);
    expect(normalizer.example().reviewBeforeMerge).toBe(true);
    rejects({ reviewBeforeMerge: 'ja' as never }, /reviewBeforeMerge muss true oder false sein/);
  });

  it('Einrichtung neuer Worktrees: Vorgabe leer und 15 min, cwd relativ und innerhalb des Worktrees, run Pflicht', () => {
    expect(normalizer.normalize({})).toMatchObject({ worktreeSetup: [], worktreeSetupTimeoutMinutes: 15 });
    expect(normalizer.example()).toMatchObject({ worktreeSetup: [], worktreeSetupTimeoutMinutes: 15 });
    expect(normalizer.normalize({
      worktreeSetup: [{ cwd: ' BackofficeNextGen\\ ', run: ' npm ci --prefer-offline --no-audit ' }, { run: 'npm run build' }, { cwd: './a/./b/', run: 'x' }],
      worktreeSetupTimeoutMinutes: 5,
    })).toMatchObject({
      worktreeSetup: [{ cwd: 'BackofficeNextGen', run: 'npm ci --prefer-offline --no-audit' }, { cwd: '.', run: 'npm run build' }, { cwd: 'a/b', run: 'x' }],
      worktreeSetupTimeoutMinutes: 5,
    });
    rejects({ worktreeSetup: [{ cwd: '../Haupt', run: 'npm ci' }] }, /worktreeSetup\[0\].*verlässt/);
    rejects({ worktreeSetup: [{ cwd: 'a/../../b', run: 'npm ci' }] }, /verlässt/);
    rejects({ worktreeSetup: [{ cwd: 'C:\\Projekte\\x', run: 'npm ci' }] }, /absolut/);
    rejects({ worktreeSetup: [{ cwd: '/tmp', run: 'npm ci' }] }, /absolut/);
    rejects({ worktreeSetup: [{ cwd: 'a', run: ' ' }] }, /run/);
    rejects({ worktreeSetup: 'npm ci' as never }, /worktreeSetup/);
    rejects({ worktreeSetupTimeoutMinutes: 0 }, /worktreeSetupTimeoutMinutes/);
  });

  it('Worktrees bündeln: Vorgabe none (ein Worktree je Ticket), sameSpec wird übernommen, Beispiel nennt sie', () => {
    expect(normalizer.normalize({}).joinWorktrees).toBe('none');
    expect(normalizer.normalize({ joinWorktrees: null as never }).joinWorktrees).toBe('none');
    expect(normalizer.normalize({ joinWorktrees: 'sameSpec' }).joinWorktrees).toBe('sameSpec');
    expect(normalizer.normalize({ joinWorktrees: ' sameSpec ' as never }).joinWorktrees).toBe('sameSpec');
    expect(normalizer.example().joinWorktrees).toBe('none');
    rejects({ joinWorktrees: 'alle' as never }, /joinWorktrees/);
    rejects({ joinWorktrees: true as never }, /joinWorktrees/);
  });

  it('lehnt Ungültiges mit Hinweis ab', () => {
    rejects({ window: { from: '25:00', to: '07:00' } }, /window/);
    rejects({ wip: -1 }, /wip/);
    rejects({ boards: ['x'] }, /boards/);
    rejects({ profiles: { a: { model: 'gpt' } } }, /Unbekanntes Modell/);
    rejects({ profiles: { a: { effort: 'ultra' } } }, /effort/);
    rejects({ profiles: { a: { model: 'fable' } } }, /Fable/);
    rejects({ defaultProfile: 'nix' }, /Profil „nix“/);
    rejects({ rules: [{ when: {}, profile: 'standard' }] }, /Bedingung/);
    rejects({ rules: [{ when: { labels: ['x'] }, profile: 'nix' }] }, /Profil „nix“/);
    rejects({ escalation: ['nix'] }, /escalation/);
  });

  it('Planungen: Vorgaben, Profil muss es geben, wip 1–5, Abnahme an, altes maxPerDay wird ignoriert', () => {
    expect(normalizer.normalize({}).planning).toEqual({ auto: false, wip: 1, acceptance: true, profile: null });
    expect(normalizer.normalize({ planning: { auto: true, wip: 3, acceptance: false, profile: 'standard' } }).planning)
      .toEqual({ auto: true, wip: 3, acceptance: false, profile: 'standard' });
    expect(normalizer.normalize({ planning: { wip: 5 } }).planning.wip).toBe(5);
    expect(normalizer.normalize({ planning: { profile: ' ' } }).planning.profile).toBeNull();
    expect(normalizer.normalize({ planning: null }).planning).toEqual({ auto: false, wip: 1, acceptance: true, profile: null });
    expect(normalizer.normalize({ planning: { maxPerDay: 0 } } as never).planning).toEqual({ auto: false, wip: 1, acceptance: true, profile: null });
    expect(normalizer.example().planning).toEqual({ auto: false, wip: 1, acceptance: true, profile: null });
    rejects({ planning: { profile: 'nix' } }, /planning\.profile: Profil „nix“/);
    rejects({ planning: { wip: 0 } }, /planning\.wip muss eine ganze Zahl von 1 bis 5/);
    rejects({ planning: { wip: 6 } }, /planning\.wip muss eine ganze Zahl von 1 bis 5/);
    rejects({ planning: 'schwer' } as never, /planning muss ein Objekt sein \(auto, wip, acceptance, profile\)/);
  });

  it('Recherchen: Vorgaben, Profil muss es geben, wip 1–5, maxPerDay ab 1', () => {
    expect(normalizer.normalize({}).research).toEqual({ auto: false, wip: 1, maxPerDay: 10, profile: null });
    expect(normalizer.normalize({ research: { auto: true, wip: 2, maxPerDay: 3, profile: 'standard' } }).research)
      .toEqual({ auto: true, wip: 2, maxPerDay: 3, profile: 'standard' });
    expect(normalizer.normalize({ research: { wip: 5 } }).research.wip).toBe(5);
    expect(normalizer.normalize({ research: { profile: '' } }).research.profile).toBeNull();
    expect(normalizer.example().research).toEqual({ auto: false, wip: 1, maxPerDay: 10 });
    rejects({ research: { profile: 'nix' } }, /research\.profile: Profil „nix“/);
    rejects({ research: { wip: 0 } }, /research\.wip muss eine ganze Zahl von 1 bis 5/);
    rejects({ research: { wip: 6 } }, /research\.wip muss eine ganze Zahl von 1 bis 5/);
    rejects({ research: { maxPerDay: 0 } }, /research\.maxPerDay/);
    rejects({ research: 'an' }, /research muss ein Objekt/);
  });

  it('erlaubt Fable nur mit allowFable', () => {
    expect(normalizer.normalize({ allowFable: true, profiles: { f: { model: 'claude-fable-5-1' } } }).profiles.f.model).toBe('claude-fable-5-1');
    expect(normalizer.modelProblem('best', false)).toMatch(/Fable/);
    expect(normalizer.modelProblem('claude-opus-5-5', false)).toBeNull();
    expect(normalizer.effortProblem('xhigh')).toBeNull();
  });
});

// Knappe eigene Vorlagen, die alle Pflichtteile ihrer Laufart erfüllen
const WORK = [
  '{{agent}} setzt {{ticketId}} („{{title}}“) um.',
  'Fertig: harness board done {{ticketId}} --by {{agent}}',
  'Blockiert: harness board set {{ticketId}} --status blocked',
  'Frage: harness board question add "…" --ticket {{ticketId}}',
].join('\n');
const PLAN = '{{agent}} plant {{ticketId}}: plan.json schreiben, dann harness board plan-json {{ticketId}}';

describe('DispatchConfigNormalizer – eigene Prompt-Vorlagen', () => {
  it('lässt ohne Angabe alle vier Laufarten auf der Standardvorlage', () => {
    expect(normalizer.normalize({}).prompts).toEqual({ work: null, plan: null, research: null, merge: null });
    expect(normalizer.normalize({ prompts: null }).prompts.work).toBeNull();
  });

  it('übernimmt eine gültige Vorlage wörtlich und lässt die anderen auf dem Standard', () => {
    const prompts = normalizer.normalize({ prompts: { work: WORK } }).prompts;
    expect(prompts.work).toBe(WORK);
    expect(prompts).toMatchObject({ plan: null, research: null, merge: null });
  });

  it('behandelt leere Vorlagen wie nicht gesetzt', () => {
    expect(normalizer.normalize({ prompts: { work: '', plan: '   ', research: null } }).prompts)
      .toEqual({ work: null, plan: null, research: null, merge: null });
  });

  it('vereinheitlicht Zeilenenden auf \\n', () => {
    const prompts = normalizer.normalize({ prompts: { plan: PLAN.replace(/: /, ':\r\n') } }).prompts;
    expect(prompts.plan).not.toContain('\r');
    expect(prompts.plan).toContain('\n');
  });

  it('fällt bei unbekanntem Platzhalter oder fehlendem Pflichtteil still auf den Standard zurück', () => {
    expect(normalizer.normalize({ prompts: { work: `${WORK}\n{{unbekannt}}` } }).prompts.work).toBeNull();
    expect(normalizer.normalize({ prompts: { work: 'nur {{agent}} und {{ticketId}}' } }).prompts.work).toBeNull();
  });

  it('nennt die Befunde über promptProblems', () => {
    const problems = normalizer.promptProblems({ prompts: { work: `${WORK}\n{{unbekannt}}`, plan: '{{agent}} ohne alles' } });
    expect(problems.map((p) => p.kind)).toEqual(['work', 'plan']);
    expect(problems[0].unknownPlaceholders).toEqual(['unbekannt']);
    expect(problems[0].missingParts).toEqual([]);
    expect(problems[1].missingParts.join(' ')).toMatch(/plan\.json/);
    expect(normalizer.promptProblems({ prompts: { work: WORK, plan: PLAN } })).toEqual([]);
  });

  it('lehnt eine falsche Struktur hart ab', () => {
    rejects({ prompts: 'text' }, /prompts muss ein Objekt sein/);
    rejects({ prompts: { arbeit: 'x' } }, /„arbeit“ ist keine Laufart/);
    expect(() => normalizer.promptProblems({ prompts: 'text' as never })).toThrow(HarnessError);
  });
});
