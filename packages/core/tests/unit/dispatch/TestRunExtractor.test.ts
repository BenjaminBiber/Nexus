import { describe, expect, it } from 'vitest';
import { TestRunExtractor } from '../../../src/dispatch/TestRunExtractor.js';

// Zeile von stream-json: Werkzeugaufruf des Assistenten
function call(id: string, command: string, name = 'Bash'): string {
  return JSON.stringify({ type: 'assistant', message: { content: [{ type: 'tool_use', id, name, input: { command } }] } });
}

// Zeile von stream-json: Ergebnis eines Werkzeugaufrufs
function result(id: string, content: unknown, isError = false): string {
  return JSON.stringify({ type: 'user', message: { content: [{ type: 'tool_result', tool_use_id: id, content, is_error: isError }] } });
}

describe('TestRunExtractor', () => {
  const extractor = new TestRunExtractor();

  it('paart Testaufrufe mit ihrem Ergebnis: grün, rot, kein Test', () => {
    const output = [
      call('a', 'npx vitest run packages/core'),
      result('a', 'Tests 12 passed'),
      call('b', 'git status --short'),
      result('b', ' M x.ts'),
      call('c', 'npm test', 'PowerShell'),
      result('c', 'FAIL x.test.ts', true),
      'keine JSON-Zeile',
    ].join('\n');
    expect(extractor.extract(output)).toEqual([
      { command: 'npx vitest run packages/core', ok: true, output: 'Tests 12 passed' },
      { command: 'npm test', ok: false, output: 'FAIL x.test.ts' },
    ]);
  });

  it('erkennt gängige Testbefehle und lässt anderes weg', () => {
    for (const cmd of [
      'npm run test:unit', 'npm run build', 'npm run lint', 'npm run typecheck', 'npx tsc --noEmit -p packages/web',
      'tsc --noEmit', 'npx jest', 'dotnet test Foo.sln', 'pytest -q', 'python -m pytest', 'go test ./...',
      'cargo test', 'mvn test', './gradlew test', 'cd web && npm test',
    ]) {
      expect(extractor.isTest(cmd), cmd).toBe(true);
    }
    for (const cmd of ['git status', 'npm ci', 'npm install', 'ls tests', 'cat vitest.config.ts', 'harness board show AIH-0001']) {
      expect(extractor.isTest(cmd), cmd).toBe(false);
    }
  });

  it('ein Aufruf ohne Ergebnis ist „ohne Ergebnis“', () => {
    expect(extractor.extract(call('a', 'npm test'))).toEqual([{ command: 'npm test', ok: null, output: '' }]);
  });

  it('kürzt die Ausgabe auf die letzten 4000 Zeichen', () => {
    const long = 'x'.repeat(5000) + 'ENDE';
    const [run] = extractor.extract([call('a', 'npm test'), result('a', long)].join('\n'));
    expect(run.output).toHaveLength(4000);
    expect(run.output.endsWith('ENDE')).toBe(true);
  });

  it('liest Ergebnisse als Liste von Textblöcken', () => {
    const output = [call('a', 'npm test'), result('a', [{ type: 'text', text: 'Teil 1' }, { type: 'text', text: 'Teil 2' }])].join('\n');
    expect(extractor.extract(output)[0].output).toBe('Teil 1\nTeil 2');
  });
});
