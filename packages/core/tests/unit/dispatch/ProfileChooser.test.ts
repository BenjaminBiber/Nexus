import { describe, expect, it } from 'vitest';
import { DispatchConfigNormalizer } from '../../../src/dispatch/DispatchConfigNormalizer.js';
import { ProfileChooser } from '../../../src/dispatch/ProfileChooser.js';
import { TimeWindowCheck } from '../../../src/dispatch/TimeWindowCheck.js';
import { AttemptCounter } from '../../../src/dispatch/AttemptCounter.js';

const normalizer = new DispatchConfigNormalizer();
const cfg = normalizer.normalize(normalizer.example());
const chooser = new ProfileChooser(cfg);

// Ticket mit Standardwerten für die Profilwahl
function ticket(extra: Partial<{ labels: string[]; priority: 'low' | 'normal' | 'high' | 'urgent'; plan: string }> = {}) {
  return { labels: [], priority: 'normal' as const, plan: '', ...extra };
}

const INTEGRATION = (rows: number) => ['## Integration', '| Datei | Änderung |', '|---|---|', ...Array.from({ length: rows }, (_, i) => `| f${i}.ts | x |`), '## Tests', '| a | b |'].join('\n');

describe('ProfileChooser', () => {
  it('nimmt ohne Hinweise das Standardprofil', () => {
    expect(chooser.choose(ticket())).toMatchObject({ profile: 'standard', model: 'opus', effort: 'high', maxTurns: 80, timeoutMinutes: 90, why: 'Standardprofil standard' });
  });

  it('Label profil: schlägt Aufwand und Regeln', () => {
    expect(chooser.choose(ticket({ labels: ['profil:klein', 'architektur'], plan: 'Aufwand: schwer' })).profile).toBe('klein');
  });

  it('liest den Aufwand des Planners als Zeile oder Abschnitt', () => {
    expect(chooser.planEstimate('**Aufwand:** Schwer – viele Dateien')).toBe('schwer');
    expect(chooser.planEstimate('## Aufwand\n\nklein, nur Doku')).toBe('klein');
    expect(chooser.planEstimate('kein Hinweis')).toBeNull();
    expect(chooser.choose(ticket({ labels: ['architektur'], plan: '## Aufwand\nklein' }))).toMatchObject({ profile: 'klein', why: 'Aufwand laut Plan: klein' });
  });

  it('nennt einen Aufwand, der kein Profil ist, und nimmt die Regeln', () => {
    const run = chooser.choose(ticket({ labels: ['doku'], plan: 'Aufwand: riesig' }));
    expect(run.profile).toBe('klein');
    expect(run.why).toBe('Regel 1 → klein (Aufwand „riesig“ im Plan ist kein Profil)');
  });

  it('wendet Regeln nach Labels, Priorität und Dateizahl an', () => {
    expect(chooser.choose(ticket({ priority: 'urgent' })).profile).toBe('schwer');
    expect(chooser.planFileCount(INTEGRATION(6))).toBe(6);
    expect(chooser.choose(ticket({ plan: INTEGRATION(6) })).profile).toBe('schwer');
    expect(chooser.choose(ticket({ plan: INTEGRATION(5) })).profile).toBe('standard');
  });

  it('eskaliert je Abbruch eine Stufe, höchstens bis zum Ende der Leiter', () => {
    expect(chooser.choose(ticket({ labels: ['doku'] }), 2)).toMatchObject({ profile: 'standard', why: 'Regel 1 → klein, Versuch 2: eskaliert auf standard' });
    expect(chooser.choose(ticket({ labels: ['doku'] }), 5).profile).toBe('schwer');
    const off = new ProfileChooser(normalizer.normalize({ ...normalizer.example(), escalate: false }));
    expect(off.choose(ticket({ labels: ['doku'] }), 3).profile).toBe('klein');
  });

  it('merkt sich das Basisprofil einer Eskalation in escalatedFrom', () => {
    expect(chooser.choose(ticket({ labels: ['doku'] }), 2).escalatedFrom).toBe('klein');
    expect(chooser.choose(ticket({ labels: ['doku'] }), 1).escalatedFrom).toBeUndefined();
    // Am Ende der Leiter bleibt das Profil gleich – keine Eskalation
    expect(chooser.choose(ticket({ labels: ['profil:schwer'] }), 3).escalatedFrom).toBeUndefined();
  });

  it('Labels model:/effort: überschreiben einzelne Werte, Haiku ohne Effort', () => {
    const run = chooser.choose(ticket({ labels: ['model:sonnet', 'effort:xhigh'] }));
    expect(run).toMatchObject({ model: 'sonnet', effort: 'xhigh' });
    expect(run.why).toBe('Standardprofil standard, Label model:sonnet, Label effort:xhigh');
    expect(chooser.choose(ticket({ labels: ['model:haiku'] })).effort).toBeNull();
  });

  it('nennt den Grund der Profilwahl und bei einer Regel ihre Nummer', () => {
    expect(chooser.choose(ticket())).toMatchObject({ reason: 'default' });
    expect(chooser.choose(ticket()).ruleIndex).toBeUndefined();
    expect(chooser.choose(ticket({ labels: ['profil:klein'] }))).toMatchObject({ reason: 'label' });
    expect(chooser.choose(ticket({ plan: '## Aufwand\nklein' }))).toMatchObject({ reason: 'plan' });
    // Regel 3 der Beispiel-Einstellung greift bei Priorität urgent
    expect(chooser.choose(ticket({ priority: 'urgent' }))).toMatchObject({ profile: 'schwer', why: 'Regel 3 → schwer', reason: 'rule', ruleIndex: 3 });
    expect(chooser.choose(ticket({ labels: ['doku'] }))).toMatchObject({ reason: 'rule', ruleIndex: 1 });
  });

  it('Eskalation und Labels model:/effort: lassen den Grund des Basisprofils stehen', () => {
    expect(chooser.choose(ticket({ labels: ['doku'] }), 2)).toMatchObject({ profile: 'standard', escalatedFrom: 'klein', reason: 'rule', ruleIndex: 1 });
    expect(chooser.choose(ticket({ labels: ['model:sonnet', 'effort:xhigh'] }))).toMatchObject({ reason: 'default' });
  });

  it('meldet unbrauchbare Labels', () => {
    expect(chooser.labelProblem(ticket({ labels: ['profil:gross'] }))).toMatch(/profil:gross/);
    expect(chooser.labelProblem(ticket({ labels: ['model:gpt'] }))).toMatch(/Unbekanntes Modell/);
    expect(chooser.labelProblem(ticket({ labels: ['model:fable'] }))).toMatch(/Fable/);
    expect(chooser.labelProblem(ticket({ labels: ['effort:mega'] }))).toMatch(/effort/);
    expect(chooser.labelProblem(ticket({ labels: ['profil:klein'] }))).toBeNull();
  });
});

describe('TimeWindowCheck', () => {
  const check = new TimeWindowCheck();
  const at = (h: number, m = 0) => new Date(2026, 9, 15, h, m);

  it('ohne Fenster immer', () => expect(check.contains(at(12), null)).toBe(true));
  it('am Tag', () => {
    expect(check.contains(at(9), { from: '08:00', to: '17:00' })).toBe(true);
    expect(check.contains(at(17), { from: '08:00', to: '17:00' })).toBe(false);
  });
  it('über Mitternacht', () => {
    const night = { from: '18:00', to: '07:00' };
    expect(check.contains(at(23), night)).toBe(true);
    expect(check.contains(at(6, 59), night)).toBe(true);
    expect(check.contains(at(12), night)).toBe(false);
  });
  it('gleiches from und to heißt immer', () => expect(check.contains(at(3), { from: '05:00', to: '05:00' })).toBe(true));
});

describe('AttemptCounter', () => {
  it('zählt nur Abbruch-Kommentare des Dispatchers', () => {
    const counter = new AttemptCounter(cfg);
    expect(counter.failed([
      { authorName: 'dispatcher', body: '[dispatch] Abbruch Versuch 1/2: x' },
      { authorName: 'dispatcher', body: '[dispatch] Start: an junior-ARC-0001' },
      { authorName: 'Florian', body: '[dispatch] Abbruch nachgemacht' },
      { authorName: 'dispatcher', body: '[dispatch] Abbruch Versuch 2/2: y' },
    ])).toBe(2);
  });
});

describe('ProfileChooser – Recherche', () => {
  it('nimmt research.profile, sonst Profil research, sonst das Standardprofil', () => {
    expect(chooser.research()).toMatchObject({ profile: 'standard', why: 'Standardprofil standard' });
    const withProfile = normalizer.normalize({ ...normalizer.example(), profiles: { ...normalizer.example().profiles, research: { model: 'sonnet', effort: 'medium' } } });
    expect(new ProfileChooser(withProfile).research()).toMatchObject({ profile: 'research', model: 'sonnet', effort: 'medium' });
    const chosen = normalizer.normalize({ ...normalizer.example(), research: { profile: 'klein' } });
    expect(new ProfileChooser(chosen).research()).toMatchObject({ profile: 'klein', why: 'Einstellung research.profile: klein' });
  });

  it('nennt als Grund setting, sobald es eine eigene Einstellung oder ein eigenes Profil gibt', () => {
    expect(chooser.research().reason).toBe('default');
    const withProfile = normalizer.normalize({ ...normalizer.example(), profiles: { ...normalizer.example().profiles, research: { model: 'sonnet', effort: 'medium' } } });
    expect(new ProfileChooser(withProfile).research().reason).toBe('setting');
    expect(new ProfileChooser(normalizer.normalize({ ...normalizer.example(), research: { profile: 'klein' } })).research().reason).toBe('setting');
  });
});

describe('ProfileChooser – Planung', () => {
  it('nimmt planning.profile, sonst Profil planning, sonst das Standardprofil', () => {
    expect(chooser.planning()).toMatchObject({ profile: 'standard', why: 'Standardprofil standard' });
    const withProfile = normalizer.normalize({ ...normalizer.example(), profiles: { ...normalizer.example().profiles, planning: { model: 'opus', effort: 'max' } } });
    expect(new ProfileChooser(withProfile).planning()).toMatchObject({ profile: 'planning', model: 'opus', effort: 'max' });
    const chosen = normalizer.normalize({ ...normalizer.example(), planning: { profile: 'schwer' } });
    expect(new ProfileChooser(chosen).planning()).toMatchObject({ profile: 'schwer', why: 'Einstellung planning.profile: schwer' });
  });

  it('nennt als Grund setting, sobald es eine eigene Einstellung oder ein eigenes Profil gibt', () => {
    expect(chooser.planning().reason).toBe('default');
    const withProfile = normalizer.normalize({ ...normalizer.example(), profiles: { ...normalizer.example().profiles, planning: { model: 'opus', effort: 'max' } } });
    expect(new ProfileChooser(withProfile).planning().reason).toBe('setting');
    expect(new ProfileChooser(normalizer.normalize({ ...normalizer.example(), planning: { profile: 'schwer' } })).planning().reason).toBe('setting');
  });
});

describe('ProfileChooser – Konfig-Version', () => {
  it('hängt den Hash der Einstellung an Arbeits-, Recherche- und Planungsläufe', () => {
    const hash = chooser.configHash;
    expect(hash).toMatch(/^[0-9a-f]{16}$/);
    expect(chooser.choose(ticket()).configHash).toBe(hash);
    expect(chooser.research().configHash).toBe(hash);
    expect(chooser.planning().configHash).toBe(hash);
  });

  it('gibt für eine geänderte Einstellung einen anderen Hash aus', () => {
    const anders = normalizer.normalize({ ...normalizer.example(), defaultProfile: 'klein' });
    expect(new ProfileChooser(anders).configHash).not.toBe(chooser.configHash);
  });
});
