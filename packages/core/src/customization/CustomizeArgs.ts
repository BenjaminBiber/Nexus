import { HarnessError } from '../errors/HarnessError.js';
import type { CopyTarget } from './CustomizationCopier.js';

export type CustomizeCommandArgs =
  | { command: 'check' }
  | { command: 'copy'; target: CopyTarget; rebase: boolean }
  | { command: 'reset'; target: CopyTarget };

const USAGE = 'Aufruf: npm run customize -- check | copy workflow|skill <name>|output-style <name> [--rebase] | reset workflow|skill <name>|output-style <name>';

// Liest die Argumente von `npm run customize -- …`.
export class CustomizeArgs {
  // Befehl mit Ziel; unvollständige, unbekannte oder überzählige Argumente werfen customize-usage mit dem Aufruftext
  parse(argv: string[]): CustomizeCommandArgs {
    const [command, ...rest] = argv;
    if (command === 'check' && rest.length === 0) return { command };
    if (command === 'copy') {
      // --rebase darf an beliebiger Stelle stehen, aber nur einmal
      const positional = rest.filter((arg) => arg !== '--rebase');
      const flags = rest.length - positional.length;
      if (flags > 1) throw this.usage();
      return { command, target: this.target(positional), rebase: flags === 1 };
    }
    if (command === 'reset') return { command, target: this.target(rest) };
    throw this.usage();
  }

  // Ziel aus genau den übrigen Argumenten: workflow, skill <name> oder output-style <name>
  private target(rest: string[]): CopyTarget {
    const [point, name] = rest;
    if (point === 'workflow' && rest.length === 1) return { point, name: 'workflow' };
    if ((point === 'skill' || point === 'output-style') && rest.length === 2 && name && !name.startsWith('-')) return { point, name };
    throw this.usage();
  }

  // Fehler mit dem Aufruftext
  private usage(): HarnessError {
    return new HarnessError('customize-usage', USAGE);
  }
}
