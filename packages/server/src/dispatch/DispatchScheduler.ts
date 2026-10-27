// Takt des Dispatchers im Server (Standard: jede Minute). Überlappende Ticks verhindert DispatchService.tick selbst.
export class DispatchScheduler {
  private timer: NodeJS.Timeout | null = null;
  /** Gerade laufender Tick des Takts (für stop) */
  private current: Promise<unknown> | null = null;

  constructor(
    private readonly service: { tick(): Promise<unknown> },
    private readonly intervalMs: number,
    private readonly log: (line: string) => void = () => undefined,
    /** So lange wartet stop höchstens auf einen laufenden Tick */
    private readonly stopWaitMs: number = 30_000,
  ) {}

  // Startet den Takt; 0 oder weniger = aus
  start(): void {
    if (this.intervalMs <= 0 || this.timer) return;
    this.timer = setInterval(() => {
      const tick = this.service.tick().catch((error: unknown) => this.log(`Dispatcher: ${(error as Error).message}`));
      this.current = tick;
      void tick.finally(() => {
        if (this.current === tick) this.current = null;
      });
    }, this.intervalMs);
    this.timer.unref();
  }

  // Hält den Takt an und wartet auf einen laufenden Tick (höchstens stopWaitMs), damit er nicht mitten im Schließen
  // des Servers auf eine schon abgebaute Board-Laufzeit trifft
  async stop(): Promise<void> {
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
    const running = this.current;
    if (!running) return;
    let waited: NodeJS.Timeout | undefined;
    const timeout = new Promise<void>((resolve) => {
      waited = setTimeout(() => {
        this.log(`Dispatcher: laufender Durchlauf nach ${Math.round(this.stopWaitMs / 1000)} s nicht beendet – Server schließt trotzdem`);
        resolve();
      }, this.stopWaitMs);
      waited.unref();
    });
    await Promise.race([running.then(() => undefined), timeout]);
    clearTimeout(waited);
  }
}
