/** Ausgang eines Laufs laut Ende seines Protokolls */
export interface RunOutcome {
  /** running = noch kein Ende im Protokoll (läuft oder wurde hart beendet) */
  state: 'finished' | 'running';
  exitCode: number | null;
  timedOut: boolean;
  endedAt: string | null;
}

const END = /\[dispatch\] (\S+) Ende: Exit-Code (-?\d+|null)(, Zeitlimit)?/g;

// Liest den Ausgang aus dem Text eines Lauf-Protokolls (Zeile „[dispatch] <ISO> Ende: Exit-Code N[, Zeitlimit]“ des Workers).
export class RunLogOutcome {
  read(text: string): RunOutcome {
    const last = [...text.matchAll(END)].at(-1);
    const limit = text.includes('[dispatch] Zeitlimit erreicht');
    if (!last) return { state: 'running', exitCode: null, timedOut: limit, endedAt: null };
    const at = new Date(last[1]);
    return {
      state: 'finished',
      exitCode: last[2] === 'null' ? null : Number(last[2]),
      timedOut: Boolean(last[3]) || limit,
      endedAt: Number.isNaN(at.getTime()) ? null : at.toISOString(),
    };
  }
}
