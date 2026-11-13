import { describe, expect, it } from 'vitest';
import { ProcessMemory } from '../lib/ProcessMemory.mjs';

describe('ProcessMemory', () => {
  it('misst den eigenen Prozess und liefert einen Wert über 0', () => {
    const mb = new ProcessMemory().rssMb(process.pid);
    expect(mb).not.toBeNull();
    expect(mb).toBeGreaterThan(0);
  });

  it('liefert null für eine Prozessnummer, die es nicht gibt', () => {
    // Nummern am oberen Rand sind praktisch nie vergeben
    expect(new ProcessMemory().rssMb(2 ** 31 - 1)).toBeNull();
  });

  it('wirft bei einer Angabe, die keine Prozessnummer ist', () => {
    expect(() => new ProcessMemory().rssBytes(0)).toThrow(/keine Prozessnummer/);
    expect(() => new ProcessMemory().rssBytes(1.5)).toThrow(/keine Prozessnummer/);
  });

  it('liest unter macOS und Linux Kilobyte aus ps', () => {
    const runner = () => ({ status: 0, stdout: ' 20480 \n' });
    expect(new ProcessMemory(runner, 'darwin').rssMb(123)).toBe(20);
  });

  it('liest unter Windows Kilobyte aus tasklist, ohne PowerShell zu starten', () => {
    const aufrufe = [];
    const runner = (file, args, options) => {
      aufrufe.push({ file, args, options });
      return { status: 0, stdout: '"node.exe","123","Console","1","51.200 K"\r\n' };
    };
    expect(new ProcessMemory(runner, 'win32').rssMb(123)).toBe(50);
    expect(aufrufe[0].file).toBe('tasklist');
    expect(aufrufe[0].args).toEqual(['/FI', 'PID eq 123', '/FO', 'CSV', '/NH']);
    expect(aufrufe[0].options.windowsHide).toBe(true);
  });

  it('versteht unter Windows Tausendertrenner jeder Sprache', () => {
    const ausgabe = (wert) => () => ({ status: 0, stdout: `"node.exe","123","Console","1","${wert}"\r\n` });
    expect(new ProcessMemory(ausgabe('51,200 K'), 'win32').rssMb(123)).toBe(50);
    expect(new ProcessMemory(ausgabe('51 200 Ko'), 'win32').rssMb(123)).toBe(50);
  });

  it('liefert unter Windows null, wenn tasklist den Prozess nicht kennt', () => {
    const keinTreffer = () => ({ status: 0, stdout: 'INFORMATION: Es werden keine Aufgaben mit den angegebenen Kriterien ausgeführt.\r\n' });
    expect(new ProcessMemory(keinTreffer, 'win32').rssMb(123)).toBeNull();
    // Eine Zeile zu einem anderen Prozess zählt nicht
    const fremd = () => ({ status: 0, stdout: '"node.exe","456","Console","1","51.200 K"\r\n' });
    expect(new ProcessMemory(fremd, 'win32').rssMb(123)).toBeNull();
  });

  it('begrenzt jeden Aufruf zeitlich und liefert null, wenn er hängt', () => {
    for (const platform of ['win32', 'linux']) {
      const optionen = [];
      const haengt = (file, args, options) => {
        optionen.push(options);
        // So meldet spawnSync ein abgelaufenes Zeitlimit
        return { status: null, signal: 'SIGTERM', stdout: '', error: new Error('spawnSync ETIMEDOUT') };
      };
      expect(new ProcessMemory(haengt, platform).rssMb(123)).toBeNull();
      expect(optionen[0].timeout).toBeGreaterThan(0);
      expect(optionen[0].stdio[0]).toBe('ignore');
    }
  });

  it('liefert null, wenn der Befehl nichts Brauchbares ausgibt', () => {
    expect(new ProcessMemory(() => ({ status: 1, stdout: '' }), 'darwin').rssMb(123)).toBeNull();
    expect(new ProcessMemory(() => ({ status: 0, stdout: 'kein Wert' }), 'darwin').rssMb(123)).toBeNull();
    expect(new ProcessMemory(() => ({ status: 0, stdout: '"node.exe","123","Console","1","0 K"' }), 'win32').rssMb(123)).toBeNull();
  });

  it('rundet auf eine Nachkommastelle', () => {
    // 1 536 KB sind 1,5 MB
    expect(new ProcessMemory(() => ({ status: 0, stdout: '1536' }), 'linux').rssMb(123)).toBe(1.5);
  });
});
