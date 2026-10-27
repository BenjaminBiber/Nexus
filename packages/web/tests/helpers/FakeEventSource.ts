// Ersatz für EventSource in Tests: Ereignisse und Verbindungsabbrüche lassen sich von Hand auslösen.
export class FakeEventSource {
  static instances: FakeEventSource[] = [];
  onopen: (() => void) | null = null;
  onmessage: ((event: { data: string }) => void) | null = null;
  onerror: (() => void) | null = null;
  closed = false;

  constructor(readonly url = '') {
    FakeEventSource.instances.push(this);
  }

  // Letzte Verbindung, die noch offen ist
  static current(): FakeEventSource {
    const open = FakeEventSource.instances.filter((i) => !i.closed);
    return open[open.length - 1];
  }

  // Setzt alle Verbindungen zurück (zwischen Tests)
  static reset(): void {
    FakeEventSource.instances = [];
  }

  // Sendet eine Meldung des Servers
  emit(message: object): void {
    this.onmessage?.({ data: JSON.stringify(message) });
  }

  // Simuliert einen Verbindungsabbruch
  fail(): void {
    this.onerror?.();
  }

  close(): void {
    this.closed = true;
  }
}
