import { describe, expect, it } from 'vitest';
import { CustomizeArgs } from '../../../src/customization/CustomizeArgs.js';

describe('CustomizeArgs', () => {
  const args = new CustomizeArgs();
  const code = (argv: string[]) => { try { args.parse(argv); return null; } catch (e) { return (e as { code?: string }).code; } };

  it('liest check, copy und reset', () => {
    expect(args.parse(['check'])).toEqual({ command: 'check' });
    expect(args.parse(['copy', 'skill', 'harness-grill', '--rebase'])).toEqual({ command: 'copy', target: { point: 'skill', name: 'harness-grill' }, rebase: true });
    expect(args.parse(['copy', 'output-style', 'kurz'])).toEqual({ command: 'copy', target: { point: 'output-style', name: 'kurz' }, rebase: false });
    expect(args.parse(['reset', 'workflow'])).toEqual({ command: 'reset', target: { point: 'workflow', name: 'workflow' } });
  });

  it('lehnt unvollständige Aufrufe ab', () => {
    for (const argv of [[], ['copy', 'skill'], ['reset', 'agent', 'x'], ['löschen']]) expect(code(argv), argv.join(' ')).toBe('customize-usage');
  });

  it('lehnt unbekannte Optionen und überzählige Argumente ab', () => {
    const invalid = [
      ['copy', 'skill', 'harness-grill', '--rebas'],
      ['copy', 'skill', '--rebas'],
      ['copy', 'skill', 'harness-grill', '--rebase', '--rebase'],
      ['reset', 'skill', 'x', '--rebase'],
      ['check', '--foo'],
      ['copy', 'workflow', 'extra'],
      ['copy', 'skill', 'harness-grill', 'extra'],
    ];
    for (const argv of invalid) expect(code(argv), argv.join(' ')).toBe('customize-usage');
  });

  it('nimmt --rebase an beliebiger Stelle', () => {
    expect(args.parse(['copy', '--rebase', 'skill', 'harness-grill'])).toEqual({ command: 'copy', target: { point: 'skill', name: 'harness-grill' }, rebase: true });
    expect(args.parse(['copy', 'workflow', '--rebase'])).toEqual({ command: 'copy', target: { point: 'workflow', name: 'workflow' }, rebase: true });
  });
});
