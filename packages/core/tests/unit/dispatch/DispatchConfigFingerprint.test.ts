import { describe, expect, it } from 'vitest';
import { DispatchConfigFingerprint } from '../../../src/dispatch/DispatchConfigFingerprint.js';
import { DispatchConfigNormalizer } from '../../../src/dispatch/DispatchConfigNormalizer.js';
import type { DispatchConfig } from '../../../src/dispatch/DispatchConfig.js';

const normalizer = new DispatchConfigNormalizer();
const fingerprint = new DispatchConfigFingerprint();
const base = () => normalizer.normalize(normalizer.example());

// Einstellung mit geänderten Feldern
function withChange(change: (cfg: DispatchConfig) => void): DispatchConfig {
  const cfg = base();
  change(cfg);
  return cfg;
}

describe('DispatchConfigFingerprint', () => {
  it('gibt 16 Hex-Zeichen zurück und bleibt für dieselbe Einstellung gleich', () => {
    const hash = fingerprint.hash(base());
    expect(hash).toMatch(/^[0-9a-f]{16}$/);
    expect(fingerprint.hash(base())).toBe(hash);
  });

  it('ist unabhängig von der Reihenfolge der Schlüssel', () => {
    const cfg = base();
    // Profile in umgekehrter Reihenfolge neu aufbauen: fachlich dieselbe Einstellung
    const profiles = Object.fromEntries(Object.entries(cfg.profiles).reverse());
    expect(fingerprint.hash({ ...cfg, profiles })).toBe(fingerprint.hash(cfg));
  });

  it('ändert sich bei Profilen, Regel-Reihenfolge, Leiter und claude-Einstellungen', () => {
    const hash = fingerprint.hash(base());
    const first = Object.keys(base().profiles)[0];
    expect(fingerprint.hash(withChange((c) => { c.profiles[first].maxTurns += 1; }))).not.toBe(hash);
    expect(fingerprint.hash(withChange((c) => { c.rules.reverse(); }))).not.toBe(hash);
    expect(fingerprint.hash(withChange((c) => { c.ladder.reverse(); }))).not.toBe(hash);
    expect(fingerprint.hash(withChange((c) => { c.maxAttempts += 1; }))).not.toBe(hash);
    expect(fingerprint.hash(withChange((c) => { c.claude.allowedTools = [...c.claude.allowedTools, 'Bash']; }))).not.toBe(hash);
    expect(fingerprint.hash(withChange((c) => { c.research.profile = 'klein'; }))).not.toBe(hash);
  });

  it('ändert sich nicht bei Fenster, WIP, Boards und Namen', () => {
    const hash = fingerprint.hash(base());
    expect(fingerprint.hash(withChange((c) => { c.wip += 3; }))).toBe(hash);
    expect(fingerprint.hash(withChange((c) => { c.window = { from: '08:00', to: '18:00' }; }))).toBe(hash);
    expect(fingerprint.hash(withChange((c) => { c.boards = [1, 2]; }))).toBe(hash);
    expect(fingerprint.hash(withChange((c) => { c.enabled = !c.enabled; }))).toBe(hash);
    expect(fingerprint.hash(withChange((c) => { c.dispatcherName = 'anders'; }))).toBe(hash);
    expect(fingerprint.hash(withChange((c) => { c.claude.command = 'C:/claude.exe'; }))).toBe(hash);
  });

  it('ändert sich mit einer eigenen Prompt-Vorlage', () => {
    const hash = fingerprint.hash(base());
    expect(base().prompts).toEqual({ work: null, plan: null, research: null, merge: null });
    expect(fingerprint.hash(withChange((c) => { c.prompts.work = 'eigene Vorlage {{agent}}'; }))).not.toBe(hash);
    expect(fingerprint.hash(withChange((c) => { c.prompts.research = 'andere Vorlage {{agent}}'; }))).not.toBe(hash);
    // Zwei verschiedene Vorlagen derselben Laufart ergeben verschiedene Hashes
    expect(fingerprint.hash(withChange((c) => { c.prompts.work = 'A {{agent}}'; })))
      .not.toBe(fingerprint.hash(withChange((c) => { c.prompts.work = 'B {{agent}}'; })));
  });

  it('liefert den Inhalt als JSON mit sortierten Schlüsseln', () => {
    const content = JSON.parse(fingerprint.content(base()));
    expect(Object.keys(content)).toEqual([...Object.keys(content)].sort());
    expect(Object.keys(content.claude)).toEqual(['allowedTools', 'effort', 'extraArgs', 'maxTurns', 'model', 'permissionMode', 'timeoutMinutes']);
    expect(content).not.toHaveProperty('wip');
    expect(content.defaultProfile).toBe(base().defaultProfile);
  });
});
