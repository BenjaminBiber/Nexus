import { Help, type Command } from 'commander';

const HEADINGS: Array<[RegExp, string]> = [
  [/^Usage:/m, 'Aufruf:'],
  [/^Options:/m, 'Optionen:'],
  [/^Commands:/m, 'Befehle:'],
  [/^Arguments:/m, 'Argumente:'],
];

// Hilfeausgabe von commander mit deutschen Zwischenüberschriften.
export class GermanHelp extends Help {
  // Übersetzt die festen englischen Überschriften der Standardausgabe
  override formatHelp(cmd: Command, helper: Help): string {
    return HEADINGS.reduce((text, [pattern, german]) => text.replace(pattern, german), super.formatHelp(cmd, helper));
  }
}
