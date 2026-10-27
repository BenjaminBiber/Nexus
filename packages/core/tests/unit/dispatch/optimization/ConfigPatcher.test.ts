import { describe, expect, it } from 'vitest';
import type { RawDispatchConfig } from '../../../../src/dispatch/DispatchConfig.js';
import { DispatchConfigNormalizer } from '../../../../src/dispatch/DispatchConfigNormalizer.js';
import { ConfigPatcher } from '../../../../src/dispatch/optimization/ConfigPatcher.js';

const normalizer = new DispatchConfigNormalizer();
const RAW: RawDispatchConfig = {
  ...normalizer.example(),
  profiles: { klein: { model: 'sonnet', maxTurns: 20, timeoutMinutes: 30 }, standard: { model: 'opus', effort: 'high', maxTurns: 30, timeoutMinutes: 60 } },
  defaultProfile: 'standard', escalation: ['klein', 'standard'], rules: [{ when: { labels: ['doku'] }, profile: 'klein' }],
  research: { auto: false, profile: null }, planning: { profile: null },
};
const CONFIG = normalizer.normalize(RAW);
const patcher = new ConfigPatcher();

describe('ConfigPatcher', () => {
  it('setzt Profilwerte, ohne andere Felder oder die Vorlage zu verändern', () => {
    const next = patcher.apply(RAW, CONFIG, [{ path: 'profiles.standard.maxTurns', from: 30, to: 45 }]);
    expect(next.profiles!.standard).toEqual({ model: 'opus', effort: 'high', maxTurns: 45, timeoutMinutes: 60 });
    expect(RAW.profiles!.standard!.maxTurns).toBe(30);
    expect(normalizer.normalize(next).profiles.standard!.maxTurns).toBe(45);
  });

  it('setzt Standardprofil, Profil einer Regel und Leiter', () => {
    const next = patcher.apply(RAW, CONFIG, [
      { path: 'defaultProfile', from: 'standard', to: 'klein' },
      { path: 'rules.0.profile', from: 'klein', to: 'standard' },
      { path: 'escalation', from: ['klein', 'standard'], to: ['standard', 'klein'] },
    ]);
    expect(next).toMatchObject({ defaultProfile: 'klein', escalation: ['standard', 'klein'] });
    expect(next.rules![0]).toEqual({ when: { labels: ['doku'] }, profile: 'standard' });
  });

  it('lehnt veraltete Ausgangswerte als Konflikt ab', () => {
    expect(() => patcher.apply(RAW, CONFIG, [{ path: 'profiles.standard.maxTurns', from: 25, to: 45 }]))
      .toThrow(expect.objectContaining({ code: 'conflict', details: { reason: 'patch-stale' } }));
  });

  it('lehnt fremde Pfade und fehlende Regeln ab', () => {
    expect(patcher.allowed('claude.command')).toBe(false);
    expect(() => patcher.apply(RAW, CONFIG, [{ path: 'claude.command', from: 'claude', to: 'x' }])).toThrow(expect.objectContaining({ code: 'input-invalid' }));
    expect(() => patcher.apply(RAW, CONFIG, [{ path: 'rules.5.profile', from: undefined, to: 'klein' }])).toThrow(expect.objectContaining({ code: 'input-invalid' }));
  });
});
