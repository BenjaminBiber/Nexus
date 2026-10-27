import { describe, it, expect, afterEach } from 'vitest';
import { TempProject } from './helpers/TempProject.js';

describe('CLI-Gerüst', () => {
  let project: TempProject;
  afterEach(() => project.dispose());

  it('zeigt die Hilfe auf Deutsch und endet mit Code 0', () => {
    project = new TempProject();
    const result = project.run(['--help']);
    expect(result.stderr).toBe('');
    expect(result.code).toBe(0);
    expect(result.stdout).toContain('AI Harness');
    expect(result.stdout).toContain('--json');
  });
});
