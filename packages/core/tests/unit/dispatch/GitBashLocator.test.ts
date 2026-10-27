import { describe, expect, it } from 'vitest';
import { GitBashLocator } from '../../../src/dispatch/GitBashLocator.js';

const locator = (files: string[], git: string | null = null) =>
  new GitBashLocator((p) => files.includes(p), () => git);

describe('GitBashLocator', () => {
  it('liefert unter anderen Systemen null', () => {
    expect(locator(['/bin/bash']).locate({ CLAUDE_CODE_GIT_BASH_PATH: '/bin/bash' }, 'linux')).toBeNull();
  });

  it('nimmt CLAUDE_CODE_GIT_BASH_PATH, wenn die Datei existiert', () => {
    expect(locator(['D:\\g\\bash.exe']).locate({ CLAUDE_CODE_GIT_BASH_PATH: 'D:\\g\\bash.exe' }, 'win32')).toBe('D:\\g\\bash.exe');
    expect(locator([]).locate({ CLAUDE_CODE_GIT_BASH_PATH: 'D:\\g\\bash.exe' }, 'win32')).toBeNull();
  });

  it.each([
    ['C:\\G\\cmd\\git.exe'],
    ['C:\\G\\mingw64\\bin\\git.exe'],
    ['C:\\G\\bin\\git.exe'],
  ])('leitet die Git-Wurzel aus %s ab', (git) => {
    expect(locator(['C:\\G\\bin\\bash.exe'], git).locate({}, 'win32')).toBe('C:\\G\\bin\\bash.exe');
  });

  it('fällt auf bekannte Orte zurück', () => {
    const local = 'C:\\Users\\u\\AppData\\Local\\Programs\\Git\\bin\\bash.exe';
    expect(locator([local]).locate({ LOCALAPPDATA: 'C:\\Users\\u\\AppData\\Local' }, 'win32')).toBe(local);
    const pf = 'C:\\Program Files\\Git\\bin\\bash.exe';
    expect(locator([pf]).locate({ ProgramFiles: 'C:\\Program Files' }, 'win32')).toBe(pf);
    expect(locator([]).locate({}, 'win32')).toBeNull();
  });
});
