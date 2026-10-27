import { HarnessError } from '../errors/HarnessError.js';
import { MAX_STEP_TEXT_LENGTH, type TicketStep } from './BoardTypes.js';
import { BoardLimits } from './BoardLimits.js';

// Ergebnis einer Schritt-Änderung: neue Liste und die Angaben für den Verlauf
export interface StepEdit {
  steps: TicketStep[];
  from: string | null;
  to: string | null;
  note: string;
}

// Änderung eines Schritts (undefined lässt das Feld unverändert, leeres Detail entfernt die Anleitung)
export interface StepChange {
  text?: string;
  detail?: string | null;
}

// Schutz vor gleichzeitigen Änderungen: erwartete Ticket-Version und/oder erwarteter Text des Schritts
export interface StepGuard {
  rev?: number;
  expectText?: string;
}

// Reine Listenoperationen für die Schritte eines Tickets (Nummern ab 1); prüft Text, Länge und Nummern.
export class TicketStepEdits {
  // Hängt einen Schritt an oder fügt ihn an Position at (1 bis Anzahl + 1) ein
  add(ticketId: string, steps: TicketStep[], text: string, options: { detail?: string | null; at?: number } = {}): StepEdit {
    const at = options.at ?? steps.length + 1;
    if (!Number.isInteger(at) || at < 1 || at > steps.length + 1) {
      throw new HarnessError('step-not-found', `${ticketId}: Position ${at} gibt es nicht (1 bis ${steps.length + 1}).`);
    }
    const step = this.withDetail({ text: this.text(text), done: false }, options.detail);
    const next = [...steps.slice(0, at - 1), step, ...steps.slice(at - 1)];
    return { steps: next, from: null, to: `${at}: ${step.text}`, note: 'Schritt hinzugefügt' };
  }

  // Ändert Text und/oder Anleitung eines Schritts; der Haken bleibt
  edit(ticketId: string, steps: TicketStep[], number: number, change: StepChange): StepEdit {
    if (change.text === undefined && change.detail === undefined) {
      throw new HarnessError('nothing-changed', 'Keine Änderung angegeben (Text oder Anleitung).');
    }
    const old = steps[this.index(ticketId, steps, number)];
    let step: TicketStep = { text: change.text === undefined ? old.text : this.text(change.text), done: old.done };
    step = this.withDetail(step, change.detail === undefined ? old.detail : change.detail);
    const next = steps.map((s, i) => (i === number - 1 ? step : s));
    const note = step.text === old.text ? 'Anleitung geändert' : 'Schritt bearbeitet';
    return { steps: next, from: `${number}: ${old.text}`, to: `${number}: ${step.text}`, note };
  }

  // Entfernt einen Schritt; die folgenden rücken nach
  remove(ticketId: string, steps: TicketStep[], number: number): StepEdit {
    const index = this.index(ticketId, steps, number);
    return { steps: steps.filter((_, i) => i !== index), from: `${number}: ${steps[index].text}`, to: null, note: 'Schritt gelöscht' };
  }

  // Verschiebt einen Schritt von Nummer from auf Nummer to
  move(ticketId: string, steps: TicketStep[], from: number, to: number): StepEdit {
    const source = this.index(ticketId, steps, from);
    const target = this.index(ticketId, steps, to);
    const next = [...steps];
    const [step] = next.splice(source, 1);
    next.splice(target, 0, step);
    return { steps: next, from: `${from}: ${step.text}`, to: `${to}: ${step.text}`, note: 'Schritt verschoben' };
  }

  // Prüft, ob an Nummer number noch der erwartete Text steht (ohne expectText keine Prüfung); sonst 'conflict'
  expect(ticketId: string, steps: TicketStep[], number: number, expectText: string | undefined): void {
    if (expectText === undefined) return;
    const step = Number.isInteger(number) && number >= 1 ? steps[number - 1] : undefined;
    if (!step) {
      throw new HarnessError('conflict', `${ticketId}: Schritt ${number} gibt es inzwischen nicht mehr – die Schritte wurden geändert.`);
    }
    if (step.text.trim() !== expectText.trim()) {
      throw new HarnessError('conflict', `${ticketId}: Schritt ${number} ist inzwischen „${step.text}“ – die Schritte wurden geändert.`);
    }
  }

  // Index einer Schrittnummer; wirft 'step-not-found' mit dem gültigen Bereich
  index(ticketId: string, steps: TicketStep[], number: number): number {
    if (!Number.isInteger(number) || number < 1 || number > steps.length) {
      throw new HarnessError('step-not-found', steps.length
        ? `${ticketId} hat keinen Schritt ${number} (1 bis ${steps.length}).`
        : `${ticketId} hat keine Schritte.`);
    }
    return number - 1;
  }

  // Text ohne Leerraum am Rand; leer oder zu lang ist nicht erlaubt
  private text(value: string): string {
    const text = (value ?? '').trim();
    if (!text) throw new HarnessError('step-empty', 'Ein Schritt braucht einen Text.');
    return BoardLimits.text(text, MAX_STEP_TEXT_LENGTH, 'Der Text eines Schritts');
  }

  // Setzt die Anleitung; leer oder null entfernt sie
  private withDetail(step: TicketStep, detail: string | null | undefined): TicketStep {
    return detail && detail.trim() ? { ...step, detail } : { text: step.text, done: step.done };
  }
}
