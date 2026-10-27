export interface GuardDenial {
  code: 'host-denied' | 'origin-denied' | 'header-missing';
  message: string;
}

type Headers = Record<string, string | string[] | undefined>;

// Schützt den lokalen Server: nur eigener Host, nur eigene Herkunft, Schreibzugriffe nur mit Pflichtkopf.
export class RequestGuard {
  private readonly hosts = new Set<string>();
  private readonly origins = new Set<string>();

  constructor(port: number) {
    this.allowPort(port);
  }

  // Erlaubt die Adressen 127.0.0.1 und localhost auf dem Port (auch nachträglich, z. B. bei Port 0)
  allowPort(port: number): void {
    for (const name of ['127.0.0.1', 'localhost']) {
      this.hosts.add(`${name}:${port}`);
      this.origins.add(`http://${name}:${port}`);
    }
  }

  // Prüft eine Anfrage; liefert null, wenn sie zulässig ist, sonst den Ablehnungsgrund
  check(method: string, headers: Headers): GuardDenial | null {
    const host = this.single(headers.host);
    if (!host || !this.hosts.has(host.toLowerCase())) {
      return { code: 'host-denied', message: 'Dieser Server antwortet nur auf 127.0.0.1 und localhost.' };
    }
    const origin = this.single(headers.origin);
    if (origin !== undefined && !this.origins.has(origin.toLowerCase())) {
      return { code: 'origin-denied', message: 'Anfragen von fremden Webseiten sind nicht erlaubt.' };
    }
    if (!this.isRead(method) && !this.hasRequiredHeaders(headers)) {
      return {
        code: 'header-missing',
        message: 'Schreibende Anfragen brauchen den Kopf X-Harness: 1 und den Inhaltstyp application/json.',
      };
    }
    return null;
  }

  // Lesende Methoden brauchen keinen Pflichtkopf
  private isRead(method: string): boolean {
    return method === 'GET' || method === 'HEAD';
  }

  // Pflichtkopf und JSON als Inhaltstyp: beides löst bei fremden Seiten eine Vorabfrage aus, die nicht beantwortet wird
  private hasRequiredHeaders(headers: Headers): boolean {
    const type = this.single(headers['content-type'])?.toLowerCase() ?? '';
    return this.single(headers['x-harness']) === '1' && type.startsWith('application/json');
  }

  // Header können Listen sein; wir verwenden den ersten Wert
  private single(value: string | string[] | undefined): string | undefined {
    return Array.isArray(value) ? value[0] : value;
  }
}
