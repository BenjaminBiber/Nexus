import { execFile } from 'node:child_process';
import { HarnessError } from '../errors/HarnessError.js';
import type { UserPath } from './UserPath.js';

// PowerShell-Skript: ergänzt den Benutzer-PATH in der Registry, ohne %VAR%-Einträge aufzulösen.
// Der Ordner kommt über die Umgebungsvariable HARNESS_BIN_DIR, damit kein Quoting nötig ist.
const SCRIPT = [
  '$d = $env:HARNESS_BIN_DIR',
  "$k = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)",
  "$p = [string]$k.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)",
  "$items = @($p -split ';' | Where-Object { $_ -ne '' })",
  "if ($items -contains $d) { 'present'; exit 0 }",
  "$k.SetValue('Path', (($items + $d) -join ';'), [Microsoft.Win32.RegistryValueKind]::ExpandString)",
  // Löscht eine nicht vorhandene Variable; löst dabei die Änderungsmeldung an neue Terminals aus
  "[Environment]::SetEnvironmentVariable('HARNESS_PATH_REFRESH', $null, 'User')",
  "'added'",
].join('; ');

// PATH-Eintrag unter Windows: Benutzer-Umgebungsvariable Path in der Registry.
export class WindowsUserPath implements UserPath {
  // Fügt den Ordner hinzu, falls er fehlt
  add(dir: string): Promise<boolean> {
    return new Promise((resolve, reject) => {
      execFile(
        'powershell.exe',
        ['-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-Command', SCRIPT],
        { env: { ...process.env, HARNESS_BIN_DIR: dir }, windowsHide: true, timeout: 20000 },
        (error, stdout) => {
          if (error) {
            reject(new HarnessError('path-register-failed', `Der PATH konnte nicht ergänzt werden: ${error.message}`));
            return;
          }
          resolve(stdout.trim() === 'added');
        },
      );
    });
  }
}
