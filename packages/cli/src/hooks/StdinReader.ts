// Liest die Hook-Eingabe von der Standardeingabe; kehrt nach `timeoutMs` auch ohne Ende der Eingabe zurück.
export class StdinReader {
  // Liefert den gelesenen Text (leer bei Terminal, Fehler oder Zeitüberschreitung)
  read(timeoutMs = 1500): Promise<string> {
    if (process.stdin.isTTY) return Promise.resolve('');
    return new Promise((resolve) => {
      const chunks: Buffer[] = [];
      const finish = () => {
        clearTimeout(timer);
        process.stdin.removeAllListeners('data');
        process.stdin.removeAllListeners('end');
        process.stdin.removeAllListeners('error');
        process.stdin.pause();
        resolve(Buffer.concat(chunks).toString('utf8'));
      };
      const timer = setTimeout(finish, timeoutMs);
      process.stdin.on('data', (chunk: Buffer) => chunks.push(Buffer.from(chunk)));
      process.stdin.on('end', finish);
      process.stdin.on('error', finish);
    });
  }
}
