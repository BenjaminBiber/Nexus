import { Command, type Help } from 'commander';
import { GermanHelp } from './GermanHelp.js';
import { GermanErrors } from './GermanErrors.js';

// Befehl mit deutscher Hilfe; gilt auch für alle Unterbefehle, die darüber angelegt werden.
export class HarnessCommand extends Command {
  constructor(name?: string) {
    super(name);
    this.helpOption('-h, --help', 'Hilfe anzeigen');
    const errors = new GermanErrors();
    this.configureOutput({ outputError: (message, write) => write(errors.translate(message)) });
  }

  // Unterbefehle erben die deutsche Hilfe
  override createCommand(name?: string): Command {
    return new HarnessCommand(name);
  }

  // Liefert die Hilfe mit deutschen Überschriften
  override createHelp(): Help {
    return Object.assign(new GermanHelp(), this.configureHelp());
  }
}
