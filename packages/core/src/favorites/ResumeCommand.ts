// Baut den kopierbaren Befehl zum Fortsetzen einer Session (der Harness startet kein Terminal).
export class ResumeCommand {
  // `cd "<Verzeichnis>" && claude --resume <Kennung>`; Anführungszeichen im Pfad werden maskiert
  static build(cwd: string, sessionId: string): string {
    const dir = `"${cwd.replace(/"/g, '\\"')}"`;
    const id = /^[A-Za-z0-9._-]+$/.test(sessionId) ? sessionId : `'${sessionId.replace(/'/g, "'\\''")}'`;
    return `cd ${dir} && claude --resume ${id}`;
  }
}
