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

  it('liest unter Windows Bytes aus WorkingSet64', () => {
    const aufrufe = [];
    const runner = (file, args, options) => {
      aufrufe.push({ file, args, options });
      return { status: 0, stdout: '52428800\r\n' };
    };
    expect(new ProcessMemory(runner, 'win32').rssMb(123)).toBe(50);
    expect(aufrufe[0].file).toBe('powershell');
    expect(aufrufe[0].args.join(' ')).toContain('WorkingSet64');
    expect(aufrufe[0].options.windowsHide).toBe(true);
  });

  it('liefert null, wenn der Befehl nichts Brauchbares ausgibt', () => {
    expect(new ProcessMemory(() => ({ status: 1, stdout: '' }), 'darwin').rssMb(123)).toBeNull();
    expect(new ProcessMemory(() => ({ status: 0, stdout: 'kein Wert' }), 'darwin').rssMb(123)).toBeNull();
    expect(new ProcessMemory(() => ({ status: 0, stdout: '0' }), 'win32').rssMb(123)).toBeNull();
  });

  it('rundet auf eine Nachkommastelle', () => {
    // 1 536 KB sind 1,5 MB
    expect(new ProcessMemory(() => ({ status: 0, stdout: '1536' }), 'linux').rssMb(123)).toBe(1.5);
  });
});
