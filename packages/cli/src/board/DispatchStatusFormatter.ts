import type { DispatchMergeView, DispatchResearchView, DispatchRunView, DispatchStatusView } from './DispatchClient.js';

const PHASES = { pending: 'Merge steht an', waiting: 'wartet auf weitere Tickets', session: 'Merge-Session läuft' } as const;

// Lesbare Zeilen für `harness board dispatch status` (wie früher `dispatch.js status`).
export class DispatchStatusFormatter {
  // Ergebnis von `harness board merge <id>` für dieses Ticket; ohne Ergebnis (älterer Server) die Übersicht
  mergeLines(view: DispatchMergeView): string[] {
    return view.result ? [view.result.text] : this.lines(view);
  }

  // Ergebnis von `harness board question research-start <id>`
  researchLines(view: DispatchResearchView): string[] {
    const r = view.started;
    return [
      `Recherche ${r.questionId ?? r.ticketId} gestartet – Profil ${r.profile} (${r.model ?? 'Standard'}/${r.effort ?? 'Standard'}), Protokoll ${r.log}.`,
      'Das Ergebnis erscheint als Recherche an der Frage; ohne Ergebnis schreibt der Dispatcher einen Kommentar.',
    ];
  }

  // Ausgabe von `harness board plan-start`
  planLines(view: DispatchResearchView): string[] {
    const r = view.started;
    return [
      `Planung ${r.ticketId} gestartet – Profil ${r.profile} (${r.model ?? 'Standard'}/${r.effort ?? 'Standard'}), Protokoll ${r.log}.`,
      'Der Plan erscheint am Ticket (danach Geplant); fehlt eine Entscheidung, legt der Agent eine Frage an; ohne Ergebnis schreibt der Dispatcher einen Kommentar.',
    ];
  }

  // Art eines Laufs für die Übersicht, z. B. „Recherche ARC-F0007“
  runLabel(r: DispatchRunView): string {
    if (r.kind === 'research') return `Recherche ${r.questionId ?? r.ticketId}`;
    if (r.kind === 'plan') return `${r.ticketId}  Planung`;
    return `${r.ticketId}  ${r.kind === 'merge' ? 'Merge-Session' : 'Umsetzung'}`;
  }

  // Zustand, laufende Worker, Merges, Bereitschaft, Fehler, Einstellungsdatei
  lines(s: DispatchStatusView): string[] {
    const out = [`Dispatcher: ${s.message}`];
    if (s.configError) out.push(`Einstellung fehlerhaft: ${s.configError}`);
    out.push('Läufe:');
    if (s.runs.length === 0) out.push('  keine');
    for (const r of s.runs) {
      out.push(`  ${this.runLabel(r)}, Profil ${r.profile} (${r.model ?? 'Standard'}/${r.effort ?? 'Standard'}), Protokoll ${r.log}`);
    }
    if (s.research) {
      const day = s.research.maxPerDay ? `, heute ${s.research.today ?? 0}/${s.research.maxPerDay} automatisch` : '';
      out.push(`Recherchen: ${s.research.active}/${s.research.wip} laufen, Automatik ${s.research.auto ? 'an' : 'aus'}${day}`);
      if (s.research.error) out.push(`  Automatik-Fehler${s.research.retryAt ? ' (Neuversuch ab ' + s.research.retryAt + ')' : ''}: ${s.research.error}`);
    }
    if (s.planningAuto) {
      out.push(`Planungen: ${s.planningAuto.active}/${s.planningAuto.wip} laufen, Automatik ${s.planningAuto.auto ? 'an' : 'aus'}`);
      if (s.planningAuto.error) out.push(`  Automatik-Fehler${s.planningAuto.retryAt ? ' (Neuversuch ab ' + s.planningAuto.retryAt + ')' : ''}: ${s.planningAuto.error}`);
    }
    if (s.merges.length) {
      out.push('Merges:');
      for (const m of s.merges) out.push(`  ${m.ticketId}  ${PHASES[m.phase]} (${m.branch}${m.target ? ` → ${m.target}` : ''})`);
    }
    if (s.boardId !== null) {
      if (!s.boardInScope) out.push(`Board ${s.boardId} gehört nicht zu den Boards des Dispatchers.`);
      if (s.ticketsError) out.push(`Tickets nicht lesbar: ${s.ticketsError}`);
      else if (s.tickets.length === 0) out.push('Keine Tickets auf Geplant.');
      else {
        out.push(`Geplant (Board ${s.boardId}):`);
        for (const t of s.tickets) out.push(`  ${t.id}  ${t.reason}`);
      }
    }
    if (s.lastErrors.length) {
      out.push('Fehler beim letzten Durchlauf:');
      for (const e of s.lastErrors) out.push(`  ${e}`);
    }
    out.push(`Einstellung: ${s.configFile}`);
    return out;
  }
}
