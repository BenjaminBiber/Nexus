import { spawn } from 'node:child_process';
import { HarnessError } from '@harness/core';
import { FOLDER_PICKER_SCRIPT } from './FolderPickerScript.js';

// Öffnet den Ordnerdialog auf diesem Rechner und liefert den gewählten Ordner; null, wenn abgebrochen
export interface FolderPicker {
  pick(startPath: string | null): Promise<string | null>;
}

// Höchstens so lange wartet der Server auf eine Auswahl (der Dialog bleibt sonst offen liegen)
const TIMEOUT_MS = 10 * 60 * 1000;

// Der Server läuft auf dem Rechner der Person: der Ordnerdialog erscheint dort und liefert den vollen Pfad,
// den ein Browser-Dateifeld aus Sicherheitsgründen nicht herausgibt. Nur unter Windows (Windows PowerShell, Explorer-Dialog).
export class WindowsFolderPicker implements FolderPicker {
  constructor(private readonly platform: string = process.platform) {}

  // Zeigt den Dialog; Abbruch ergibt null, ein Fehler folder-picker-unavailable
  pick(startPath: string | null): Promise<string | null> {
    if (this.platform !== 'win32') {
      return Promise.reject(new HarnessError('folder-picker-unavailable', 'Die Ordnerauswahl gibt es nur unter Windows; den Pfad bitte eintippen.'));
    }
    return new Promise((resolve, reject) => {
      const unavailable = (detail: string) => new HarnessError('folder-picker-unavailable', `Die Ordnerauswahl ließ sich nicht öffnen (${detail}); den Pfad bitte eintippen.`);
      const encoded = Buffer.from(FOLDER_PICKER_SCRIPT, 'utf16le').toString('base64');
      const args = ['-NoProfile', '-NonInteractive', '-STA', '-ExecutionPolicy', 'Bypass', '-EncodedCommand', encoded];
      const env = { ...process.env, HARNESS_PICK_START: startPath ?? '' };
      const child = spawn('powershell.exe', args, { env, windowsHide: false, stdio: ['ignore', 'pipe', 'ignore'] });
      const timer = setTimeout(() => child.kill(), TIMEOUT_MS);
      const chunks: Buffer[] = [];
      child.stdout.on('data', (chunk: Buffer) => chunks.push(chunk));
      child.on('error', (error) => {
        clearTimeout(timer);
        reject(unavailable(error.message));
      });
      child.on('exit', (code) => {
        clearTimeout(timer);
        if (code !== 0) return reject(unavailable(`PowerShell endete mit Code ${code}`));
        const path = Buffer.concat(chunks).toString('utf8').trim();
        resolve(path === '' ? null : path);
      });
    });
  }
}
