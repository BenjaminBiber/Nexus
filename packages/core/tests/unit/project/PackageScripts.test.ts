import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

/** Liest die Skripte der Wurzel-package.json. */
function leseWurzelSkripte(): Record<string, string> {
  const pfad = fileURLToPath(new URL('../../../../../package.json', import.meta.url));
  const inhalt = JSON.parse(readFileSync(pfad, 'utf8')) as { scripts?: Record<string, string> };
  return inhalt.scripts ?? {};
}

describe('Wurzel-package.json', () => {
  it('stellt ein lint-Skript bereit, das die Typprüfung ausführt', () => {
    const skripte = leseWurzelSkripte();
    expect(skripte.lint?.trim()).toBeTruthy();
    expect(skripte.lint).toMatch(/typecheck|tsc/);
  });

  it('stellt ein typecheck-Skript bereit', () => {
    const skripte = leseWurzelSkripte();
    expect(skripte.typecheck?.trim()).toBeTruthy();
  });
});
