import { describe, expect, it } from 'vitest';
import { splitCommand } from '../lib/BenchProject.mjs';

describe('splitCommand', () => {
  it('trennt Programm und Argumente', () => {
    expect(splitCommand('nexus hook')).toEqual(['nexus', 'hook']);
  });

  it('hält Teile in Anführungszeichen zusammen und entfernt sie', () => {
    expect(splitCommand('node "/pfad mit leerzeichen/cli.mjs" hook')).toEqual(['node', '/pfad mit leerzeichen/cli.mjs', 'hook']);
  });

  it('übergeht zusätzlichen Leerraum', () => {
    expect(splitCommand('  nexus   hook  ')).toEqual(['nexus', 'hook']);
  });

  it('wirft bei einem leeren Befehl', () => {
    expect(() => splitCommand('   ')).toThrow(/leer/);
  });
});
