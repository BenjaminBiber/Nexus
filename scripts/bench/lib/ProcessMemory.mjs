import { spawnSync } from 'node:child_process';

// Obergrenze je Abfrage; ein hängender Befehl ergibt null statt den Aufrufer zu blockieren
const TIMEOUT_MS = 30000;

// Kindprozess ohne Eingabe und ohne Konsolenfenster, damit im Hintergrund nichts aufblitzt
const SPAWN_OPTIONS = { encoding: 'utf8', windowsHide: true, timeout: TIMEOUT_MS, stdio: ['ignore', 'pipe', 'ignore'] };

// Speicherbedarf eines fremden Prozesses in MB (Resident Set Size). Unter Windows über `tasklist`, sonst über `ps`.
export class ProcessMemory {
  // runner nur für Tests austauschbar; Standard ist spawnSync
  constructor(runner = spawnSync, platform = process.platform) {
    this.runner = runner;
    this.platform = platform;
  }

  // RSS in MB mit einer Nachkommastelle; null, wenn der Prozess nicht (mehr) zu finden ist
  rssMb(pid) {
    const bytes = this.rssBytes(pid);
    return bytes === null ? null : Math.round((bytes / 1024 / 1024) * 10) / 10;
  }

  // RSS in Bytes oder null
  rssBytes(pid) {
    if (!Number.isInteger(pid) || pid <= 0) throw new Error(`„${pid}“ ist keine Prozessnummer.`);
    return this.platform === 'win32' ? this.windows(pid) : this.unix(pid);
  }

  // macOS und Linux: `ps -o rss= -p <pid>` liefert Kilobyte
  unix(pid) {
    const result = this.runner('ps', ['-o', 'rss=', '-p', String(pid)], SPAWN_OPTIONS);
    const kb = Number((result.stdout ?? '').trim());
    return result.status === 0 ? positiveKb(kb) : null;
  }

  // Windows: Arbeitsspeicher (Working Set) in KB aus `tasklist`. Kein PowerShell: dessen Kaltstart lädt .NET
  // und braucht auf einer frischen CI-Maschine mitunter Minuten.
  windows(pid) {
    const result = this.runner('tasklist', ['/FI', `PID eq ${pid}`, '/FO', 'CSV', '/NH'], SPAWN_OPTIONS);
    if (result.status !== 0) return null;
    // Zeilen wie "node.exe","123","Console","1","51.200 K"; ohne Treffer nur ein Hinweistext
    for (const line of (result.stdout ?? '').split(/\r?\n/)) {
      const fields = line.trim().replace(/^"|"$/g, '').split('","');
      if (fields.length < 5 || fields[1] !== String(pid)) continue;
      // Tausendertrenner und Einheit hängen von der Sprache ab; es zählen nur die Ziffern
      const digits = fields[fields.length - 1].replace(/\D/g, '');
      return digits === '' ? null : positiveKb(Number(digits));
    }
    return null;
  }
}

// Kilobyte als Bytes; null für alles, was kein positiver Wert ist
function positiveKb(kb) {
  return Number.isFinite(kb) && kb > 0 ? kb * 1024 : null;
}
