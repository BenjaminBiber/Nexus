import { spawnSync } from 'node:child_process';

// Speicherbedarf eines fremden Prozesses in MB (Resident Set Size). Unter Windows über PowerShell, sonst über `ps`.
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
    const result = this.runner('ps', ['-o', 'rss=', '-p', String(pid)], { encoding: 'utf8' });
    const kb = Number((result.stdout ?? '').trim());
    if (result.status !== 0 || !Number.isFinite(kb) || kb <= 0) return null;
    return kb * 1024;
  }

  // Windows: WorkingSet64 in Bytes; ohne Konsolenfenster, damit im Hintergrund nichts aufblitzt
  windows(pid) {
    const command = `$p = Get-Process -Id ${pid} -ErrorAction SilentlyContinue; if ($p) { $p.WorkingSet64 }`;
    const result = this.runner('powershell', ['-NoProfile', '-Command', command], { encoding: 'utf8', windowsHide: true });
    const bytes = Number((result.stdout ?? '').trim());
    if (result.status !== 0 || !Number.isFinite(bytes) || bytes <= 0) return null;
    return bytes;
  }
}
