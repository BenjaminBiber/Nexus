import { DEFAULT_SERVER_PORT, HarnessError } from '@harness/core';

// Prüft die Optionen der Dispatcher-Befehle
export class DispatchOptionParser {
  // Board-ID aus der Option
  boardId(value?: string): number | undefined {
    if (value === undefined) return undefined;
    const id = Number(value);
    if (!Number.isInteger(id) || id < 1) throw new HarnessError('id-invalid', `„${value}“ ist keine Board-ID.`);
    return id;
  }

  // Dauer in Minuten, ganze Zahl von 1 bis 1440
  minutes(value: string): number {
    const minutes = Number(value);
    if (!/^\d+$/.test(value) || minutes < 1 || minutes > 1440) throw new HarnessError('input-invalid', 'minutes muss eine ganze Zahl von 1 bis 1440 sein.');
    return minutes;
  }

  // Port: Option, sonst HARNESS_PORT, sonst Standard
  port(option?: string, env: string | undefined = process.env.HARNESS_PORT): number {
    const raw = option ?? env ?? String(DEFAULT_SERVER_PORT);
    const port = Number(raw);
    if (!Number.isInteger(port) || port < 1 || port > 65535) throw new HarnessError('port-invalid', `Der Port „${raw}“ ist ungültig.`);
    return port;
  }
}
