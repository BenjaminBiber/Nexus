import { describe, it, expect } from 'vitest';
import { TargetBranchRule } from '../../../src/git/TargetBranchRule.js';

describe('TargetBranchRule', () => {
  const rule = new TargetBranchRule();

  it('lässt gültige Namen durch und leert Leerraum', () => {
    expect(rule.normalize(' release/2026.10 ')).toBe('release/2026.10');
    expect(rule.normalize('feature/zielbranch')).toBe('feature/zielbranch');
    expect(rule.normalize('')).toBeNull();
    expect(rule.normalize('   ')).toBeNull();
    expect(rule.normalize(null)).toBeNull();
    expect(rule.normalize(undefined)).toBeNull();
  });

  it.each(['-x', 'a..b', 'a b', 'a~b', 'a^b', 'a:b', 'a?b', 'a*b', 'a[b', 'a\\b', 'a@{b', '@', '.a', 'a/.b', 'a.lock', 'a/b.lock/c', 'a/', 'a.', 'a//b', 'HEAD', 'ticket/ARC-0001-x', 'x'.repeat(401)])(
    'lehnt „%s“ ab', (name) => {
      expect(() => rule.normalize(name)).toThrow(expect.objectContaining({ code: expect.stringMatching(/^(target-branch-invalid|value-too-long)$/) }));
    });

  it('nennt bei ticket/ den Grund', () => {
    expect(() => rule.normalize('ticket/ARC-1')).toThrow(/Branch eines Tickets/);
  });

  it.each(['ticket', 'Ticket/ARC-1', 'TICKET/x'])('lehnt „%s“ als Ticket-Branch ab (Konflikt mit refs/heads/ticket/*, Groß-/Kleinschreibung unter Windows)', (name) => {
    expect(() => rule.normalize(name)).toThrow(expect.objectContaining({ code: 'target-branch-invalid', message: expect.stringMatching(/Branch eines Tickets/) }));
  });

  it.each(['tickets/x', 'my-ticket/x'])('lässt „%s“ durch', (name) => {
    expect(rule.normalize(name)).toBe(name);
  });
});
