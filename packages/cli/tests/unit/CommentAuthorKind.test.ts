import { describe, expect, it } from 'vitest';
import { CommentAuthorKind } from '../../src/board/CommentAuthorKind.js';

describe('CommentAuthorKind', () => {
  const kind = new CommentAuthorKind();

  it('erkennt feste Agentennamen und Präfixe', () => {
    for (const name of ['claude', 'Claude', 'dispatcher', 'junior-AIH-0070', 'planner-AIH-0042', 'research-ARC-F0001']) {
      expect(kind.isAgent(name)).toBe(true);
    }
  });

  it('hält andere Namen für Menschen', () => {
    for (const name of ['Florian', 'benjamin.biber', '', 'juniorin']) {
      expect(kind.isAgent(name)).toBe(false);
    }
  });
});
