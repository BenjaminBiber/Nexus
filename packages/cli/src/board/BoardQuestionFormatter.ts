import { QUESTION_STATE_LABELS, type BoardQuestionDetail, type BoardQuestionView } from '@harness/core';

const KIND: Record<string, string> = { human: 'Mensch', session: 'Session', subagent: 'Subagent' };

// Textausgaben der Fragen für Menschen (Text) und Agenten (Markdown).
export class BoardQuestionFormatter {
  // Eine Zeile je Frage: „  ARC-F0001  Titel  [bereit] [wartet auf …] [3 Optionen] → ARC-0007“
  listText(questions: BoardQuestionView[]): string[] {
    if (!questions.length) return ['(keine Fragen gefunden)'];
    return questions.map((q) => {
      const marks = this.marks(q);
      return `  ${q.id}  ${q.title}${marks.length ? `  ${marks.join(' ')}` : ''}`;
    });
  }

  // Markdown mit Zustand, betroffenen Tickets, offenen Abhängigkeiten und Recherche
  listMarkdown(questions: BoardQuestionView[]): string[] {
    const lines = ['## Fragen'];
    if (!questions.length) return [...lines, '', '- (keine Fragen gefunden)'];
    lines.push('');
    for (const q of questions) {
      lines.push(`- **${q.id}** ${q.title} — ${QUESTION_STATE_LABELS[q.state]}${q.ready ? ', bereit zur Entscheidung' : ''}`);
      if (q.ticketIds.length) lines.push(`  - Betrifft: ${q.ticketIds.join(', ')}`);
      if (q.waitingOn.length) lines.push(`  - Wartet auf: ${q.waitingOn.map((w) => w.id).join(', ')}`);
      if (q.researched) lines.push(`  - Neueste Recherche: ${q.optionCount === 1 ? '1 Option' : `${q.optionCount} Optionen`}`);
    }
    return lines;
  }

  // Eine Frage mit Beschreibung, Recherchen, Entscheidung, Kommentaren und Verlauf
  showText(q: BoardQuestionDetail): string[] {
    const list = (items: string[]) => (items.length ? items.join(', ') : '–');
    const lines = [
      `${q.id}  ${q.title}`,
      `Zustand: ${QUESTION_STATE_LABELS[q.state]}${q.ready ? ' (bereit zur Entscheidung)' : ''} | rev ${q.rev}`,
      `Tags: ${list(q.tags)}`,
      `Herkunft: ${q.originName} (${KIND[q.originKind] ?? q.originKind})${q.originSession ? `, Session ${q.originSession}` : ''}`,
      `Anlass: ${q.occasion ?? '–'}`,
      `Betrifft: ${list(q.tickets.map((t) => `${t.id} ${t.title}`))}`,
      `Abhängig von: ${list(q.dependsOn)}`,
      `Wartet auf: ${list(q.waitingOn.map((w) => `${w.id} (${w.state})`))}`,
      '',
      q.description.trimEnd() || '(keine Beschreibung)',
      '',
    ];
    if (!q.research.length) lines.push('Recherchen: noch keine', '');
    else {
      lines.push('Recherchen:');
      for (const r of q.research) {
        lines.push(`  [${r.at}] ${r.byName} (${KIND[r.kind] ?? r.kind})`);
        if (r.context.trim()) lines.push(...r.context.trimEnd().split('\n').map((l) => `    ${l}`));
        for (const o of r.options) {
          lines.push(`    ${o.id}${o.recommended ? ' (empfohlen)' : ''}: ${o.title} – ${o.text}`);
          o.pros.forEach((p) => lines.push(`      + ${p}`));
          o.cons.forEach((c) => lines.push(`      - ${c}`));
        }
      }
      lines.push('');
    }
    if (q.decision) {
      const d = q.decision;
      lines.push(`${d.outcome === 'answered' ? 'Entscheidung' : 'Verworfen'}: ${d.option ?? d.text ?? '–'}`);
      lines.push(`  Begründung: ${d.reason} (${d.byName}, ${d.at})`, '');
    }
    lines.push('Kommentare:');
    if (!q.comments.length) lines.push('  (keine)');
    for (const c of q.comments) lines.push(`  [${c.createdAt}] ${c.authorName} (${KIND[c.authorKind] ?? c.authorKind}): ${c.body}`);
    lines.push('', 'Verlauf:');
    for (const h of q.history) lines.push(`  [${h.at}] ${h.byName}${h.byKind ? ` (${KIND[h.byKind] ?? h.byKind})` : ''} ${h.type}: ${h.text}`);
    return lines;
  }

  // Kennzeichen einer Frage in der Textliste
  private marks(q: BoardQuestionView): string[] {
    const marks: string[] = [];
    if (q.state !== 'open') marks.push(`[${QUESTION_STATE_LABELS[q.state]}]`);
    if (q.ready) marks.push('[bereit]');
    if (q.waitingOn.length) marks.push(`[wartet auf ${q.waitingOn.map((w) => w.id).join(', ')}]`);
    if (q.researched) marks.push(`[${q.optionCount === 1 ? '1 Option' : `${q.optionCount} Optionen`}]`);
    if (q.ticketIds.length) marks.push(`→ ${q.ticketIds.join(', ')}`);
    return marks;
  }
}
