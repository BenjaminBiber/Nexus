import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { TempRepo } from '../helpers/TempRepo.js';
import { SpecCommitCheck } from '../../../src/docs/SpecCommitCheck.js';

const DOC = 'docs/superpowers/specs/2026-10-22-login-design.md';

describe('SpecCommitCheck', () => {
  let repo: TempRepo;
  const check = new SpecCommitCheck();
  const commit = () => {
    repo.git('-c', 'core.autocrlf=false', 'add', '-A');
    repo.git('-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '-q', '-m', 'spec');
  };

  beforeEach(() => {
    repo = new TempRepo();
    mkdirSync(join(repo.path, 'docs', 'superpowers', 'specs'), { recursive: true });
    writeFileSync(join(repo.path, DOC), '# Login\n');
  });
  afterEach(() => repo.dispose());

  it('warnt bei einem ungetrackten Dokument (fehlt im HEAD)', () => {
    expect(check.warningForDoc(repo.path, DOC)).toBe(
      `Das Spec-Dokument ${DOC} ist nicht (vollständig) committet – Worktrees und Agenten sehen nur den letzten Commit. Erst committen.`,
    );
  });

  it('schweigt bei einem vollständig committeten Dokument', () => {
    commit();
    expect(check.warningForDoc(repo.path, DOC)).toBeNull();
  });

  it('warnt bei einem geänderten Dokument', () => {
    commit();
    writeFileSync(join(repo.path, DOC), '# Login\n\nNeuer Abschnitt\n');
    expect(check.warningForDoc(repo.path, DOC)).toContain('nicht (vollständig) committet');
  });

  it('übergeht Änderungen an anderen Dateien', () => {
    commit();
    writeFileSync(join(repo.path, 'README.md'), 'neu');
    expect(check.warningForDoc(repo.path, DOC)).toBeNull();
  });

  it('schweigt bei fehlender Datei, leerem Pfad und ohne Git-Repo', () => {
    expect(check.warningForDoc(repo.path, 'docs/superpowers/specs/gibt-es-nicht.md')).toBeNull();
    expect(check.warningForDoc(repo.path, '')).toBeNull();
    const plain = mkdtempSync(join(tmpdir(), 'harness-no-git-'));
    try {
      writeFileSync(join(plain, 'spec.md'), '# x\n');
      expect(check.warningForDoc(plain, 'spec.md')).toBeNull();
    } finally {
      rmSync(plain, { recursive: true, force: true });
    }
  });
});
