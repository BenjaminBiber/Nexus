import { spawn } from 'node:child_process';

export const TOAST_APP_ID = 'AI.Harness';

// Führt ein PowerShell-Skript mit Umgebungsvariablen aus; liefert den Exitcode
export type ScriptRunner = (script: string, env: Record<string, string>) => Promise<number>;

// Meldet „AI Harness“ als App an (Name und Symbol im Toast) und zeigt den Toast aus der Umgebungsvariable
const SCRIPT = `
$ErrorActionPreference = 'Stop'
$key = 'HKCU:\\Software\\Classes\\AppUserModelId\\' + $env:HARNESS_TOAST_APPID
New-Item -Path $key -Force | Out-Null
Set-ItemProperty -Path $key -Name DisplayName -Value 'AI Harness'
Set-ItemProperty -Path $key -Name IconUri -Value $env:HARNESS_TOAST_ICON
Set-ItemProperty -Path $key -Name ShowInSettings -Value 0 -Type DWord
[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
[Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null
$xml = New-Object Windows.Data.Xml.Dom.XmlDocument
$xml.LoadXml($env:HARNESS_TOAST_XML)
$toast = New-Object Windows.UI.Notifications.ToastNotification $xml
$toast.Tag = $env:HARNESS_TOAST_TAG
$toast.Group = 'harness'
[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier($env:HARNESS_TOAST_APPID).Show($toast)
`;

// Zeigt einen Toast über Windows PowerShell (WinRT); meldet sich in Windows als „AI Harness“ an.
export class WindowsToastSender {
  constructor(
    private readonly run: ScriptRunner = WindowsToastSender.powershell,
    private readonly platform: string = process.platform,
  ) {}

  // Zeigt den Toast; Fehler als Text, sonst null (auf anderen Systemen wird nichts gezeigt).
  // Gleiche `tag` ersetzen sich; Fragen bekommen eigene, damit sie Hinweise auf wartende Sessions nicht verdrängen.
  async show(xml: string, iconPath: string, tag = 'attention'): Promise<string | null> {
    if (this.platform !== 'win32') return 'Windows-Benachrichtigungen gibt es nur unter Windows.';
    try {
      const env = { HARNESS_TOAST_XML: xml, HARNESS_TOAST_ICON: iconPath, HARNESS_TOAST_APPID: TOAST_APP_ID, HARNESS_TOAST_TAG: tag };
      const code = await this.run(SCRIPT, env);
      return code === 0 ? null : `PowerShell endete mit Code ${code}.`;
    } catch (error) {
      return (error as Error).message;
    }
  }

  // Standard: Windows PowerShell mit kodiertem Skript, ohne Fenster
  private static powershell(script: string, env: Record<string, string>): Promise<number> {
    return new Promise((resolve, reject) => {
      const encoded = Buffer.from(script, 'utf16le').toString('base64');
      const args = ['-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-EncodedCommand', encoded];
      const child = spawn('powershell.exe', args, { env: { ...process.env, ...env }, windowsHide: true, stdio: 'ignore' });
      child.on('error', reject);
      child.on('exit', (code) => resolve(code ?? 1));
    });
  }
}
