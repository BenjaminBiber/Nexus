import { DB_COLUMN_LABELS, DB_QUESTION_STATE_LABELS, type DbColumnId, type DbTimelineEventDto } from '../api/dbTypes.js';

// Farbe des Punkts: Erfolg, Fehler, Hinweis (Mensch/Start), Warten, neutral
export type TimelineTone = 'ok' | 'error' | 'info' | 'warn' | 'plain';

// Anzeige eines Ereignisses: Titelzeile, Metazeile, optional Fehler- oder Detailkasten und Link auf eine Frage
export interface TimelineItemView {
  tone: TimelineTone;
  title: string;
  meta: string[];
  // Hervorgehoben (grauer Kasten): Läufe, Kommentare, Merge, Entscheidungen, Fehler, Erledigt/Blockiert
  highlight: boolean;
  error?: string;
  // Gelber Kasten: wartet auf einen Menschen (Human Interaction)
  warning?: string;
  detail?: string;
  // Kommentartext als Markdown
  markdown?: string;
  questionId?: string;
}

// Feldnamen des Verlaufs in deutscher Schreibweise
export const FIELD_LABELS: Record<string, string> = {
  status: 'Status', title: 'Titel', priority: 'Priorität', assignee: 'Zuständig', labels: 'Labels', dependsOn: 'Voraussetzungen',
  needsPlanning: 'Planungsbedarf', blockedReason: 'Blockiert-Grund', acceptance: 'Abnahme', description: 'Beschreibung',
  plan: 'Plan', handbook: 'Handbuch', diagram: 'Diagramm', boardId: 'Board', question: 'Frage', plannedBy: 'Geplant von',
  planningClaimBy: 'Planungs-Claim',
  spec: 'Spec', specTask: 'Spec-Aufgabe', branch: 'Branch', worktree: 'Worktree', targetBranch: 'Zielbranch', touches: 'Bereiche', consumes: 'Nutzt', produces: 'Liefert', steps: 'Schritte', step: 'Schritt',
};

const LONG_TEXT = new Set(['description', 'plan', 'handbook', 'diagram']);
const LISTS = new Set(['labels', 'dependsOn', 'touches', 'consumes', 'produces']);
const RUN_LABEL: Record<string, string> = { 'Umsetzung': 'Umsetzung', 'Merge-Session': 'Merge-Session', 'Recherche': 'Recherche' };

// Status in deutscher Schreibweise
function statusLabel(value: string | null | undefined): string {
  if (!value) return '–';
  return DB_COLUMN_LABELS[value as DbColumnId] ?? value;
}

// Fragenzustand in deutscher Schreibweise
function questionState(value: string | null | undefined): string {
  if (!value) return '–';
  return (DB_QUESTION_STATE_LABELS as Record<string, string>)[value] ?? value;
}

// JSON-Liste aus dem Verlauf als kommagetrennter Text
function list(value: string | null | undefined): string {
  if (!value) return '–';
  try {
    const items = JSON.parse(value) as unknown;
    return Array.isArray(items) && items.length ? items.join(', ') : '–';
  } catch {
    return value;
  }
}

// Text eines Dispatcher-Kommentars ohne Marker („[dispatch] Abbruch Versuch 1/2: …“ → „Versuch 1/2: …“)
function afterMarker(text: string | undefined, marker: string): string {
  const rest = (text ?? '').startsWith(marker) ? (text ?? '').slice(marker.length) : text ?? '';
  return rest.replace(/^[\s:–-]+/, '').trim();
}

// Wer gehandelt hat, für die Metazeile
function who(e: DbTimelineEventDto): string {
  return `${e.actor.name} · ${e.actor.type === 'agent' ? 'Agent' : 'Person'}`;
}

// Ereignis eines Verlaufsfelds
function fieldView(e: DbTimelineEventDto): TimelineItemView {
  const field = e.field ?? '';
  const label = FIELD_LABELS[field] ?? field;
  const meta = [who(e), ...(e.note ? [e.note] : [])];
  const base = { meta, highlight: false, tone: 'plain' as TimelineTone };
  switch (field) {
    case 'status': {
      const tone: TimelineTone = e.to === 'done' ? 'ok' : e.to === 'blocked' ? 'error' : 'plain';
      const title = e.from ? `${statusLabel(e.from)} → ${statusLabel(e.to)}` : `Angelegt in ${statusLabel(e.to)}`;
      return { ...base, tone, title, highlight: e.to === 'done' || e.to === 'blocked' };
    }
    case 'blockedReason':
      if (!e.to) return { ...base, title: 'Blockiert-Grund entfernt', detail: e.from ?? undefined };
      if (e.to.startsWith('Human Interaction')) return { ...base, tone: 'warn', title: 'Wartet auf Abnahme', warning: e.to, highlight: true };
      return { ...base, tone: 'error', title: 'Blockiert', error: e.to, highlight: true };
    case 'assignee':
      return { ...base, title: e.to ? `Übernommen von ${e.to}` : `Freigegeben (war ${e.from ?? '–'})` };
    case 'planningClaimBy':
      return { ...base, title: e.to ? `Planung beansprucht von ${e.to}` : 'Planungs-Claim aufgehoben' };
    case 'question':
      return { ...base, title: `Frage ${questionState(e.from)} → ${questionState(e.to)}`, questionId: e.ref?.questionId };
    case 'step':
      return { ...base, title: `Schritt ${e.to ?? e.from ?? ''}`, meta: [who(e), ...(e.note && e.note !== e.to ? [e.note] : [])] };
    case 'acceptance':
      return { ...base, title: 'Abnahme geändert', detail: e.to ?? undefined };
    case 'branch':
      return { ...base, title: e.to ? `Branch ${e.to}` : 'Branch entfernt' };
    case 'worktree':
      return { ...base, title: e.to ? `Worktree ${e.to}` : 'Worktree entfernt' };
    case 'needsPlanning':
      return { ...base, title: e.to === 'true' ? 'Braucht Planung' : 'Planung erledigt' };
  }
  if (LONG_TEXT.has(field)) {
    const created = e.from === '0 Zeichen';
    return { ...base, title: `${label} ${created ? 'geschrieben' : e.to === '0 Zeichen' ? 'geleert' : 'geändert'}`, meta: [who(e), `${e.from ?? '–'} → ${e.to ?? '–'}`, ...(e.note ? [e.note] : [])] };
  }
  if (LISTS.has(field)) return { ...base, title: `${label}: ${list(e.to)}`, meta: [who(e), `vorher: ${list(e.from)}`, ...(e.note ? [e.note] : [])] };
  return { ...base, title: `${label}: ${e.from ?? '–'} → ${e.to ?? '–'}` };
}

// Ereignis eines Laufs, eines Merges, einer Frage, eines Commits oder Kommentars; Titel beginnen groß (Namen und Commit-Betreff bleiben)
export function timelineView(e: DbTimelineEventDto): TimelineItemView {
  const v = rawView(e);
  if (e.type === 'comment' || e.type === 'commit') return v;
  return { ...v, title: v.title.charAt(0).toUpperCase() + v.title.slice(1) };
}

function rawView(e: DbTimelineEventDto): TimelineItemView {
  if (e.field) return fieldView(e);
  const meta = [who(e)];
  const text = e.text ?? '';
  switch (e.type) {
    case 'comment':
      return { tone: e.actor.type === 'person' ? 'info' : 'plain', title: `${e.actor.name}:`, meta: [e.actor.type === 'agent' ? 'Agent' : 'Person'], markdown: text, highlight: true };
    case 'dispatch:start': {
      const agent = /an (\S+) übergeben/.exec(text)?.[1];
      const branch = /Branch (\S+?),/.exec(text)?.[1];
      return {
        tone: 'info', title: `Lauf gestartet${e.ref?.profile ? ` – Profil ${e.ref.profile}` : ''}`, highlight: true,
        meta: [who(e), ...(agent ? [`an ${agent}`] : []), ...(branch ? [`Branch ${branch}`] : [])],
        detail: /Grund: (.+)$/s.exec(text)?.[1]?.trim(),
      };
    }
    case 'dispatch:abort': {
      const attempt = /Versuch (\d+\/\d+)/.exec(text)?.[1];
      return { tone: 'error', title: `Lauf abgebrochen${attempt ? ` (Versuch ${attempt})` : ''}`, meta, highlight: true, error: afterMarker(text, '[dispatch] Abbruch').replace(/^Versuch \d+\/\d+:\s*/, '') };
    }
    case 'dispatch:pause':
      return { tone: 'warn', title: 'Dispatcher pausiert', meta, highlight: true, detail: afterMarker(text, '[dispatch] Pause') };
    case 'dispatch:question':
      return { tone: 'warn', title: 'Wartet auf Frage', meta, highlight: true, detail: afterMarker(text, '[dispatch] Frage') };
    case 'dispatch:setup':
      return { tone: 'error', title: 'Einrichtung des Worktrees fehlgeschlagen', meta, highlight: true, error: afterMarker(text, '[dispatch] Einrichtung') };
    case 'dispatch:review':
      return { tone: 'warn', title: 'Wartet auf Freigabe vor dem Merge', meta, highlight: true, detail: afterMarker(text, '[dispatch] Review') };
    case 'dispatch:review-return':
      return { tone: 'info', title: 'Zurück an den Agenten', meta, highlight: true, detail: afterMarker(text, '[dispatch] Zurück an den Agenten') };
    case 'merge': {
      const target = /Gemergt in (.+?) \(/.exec(text)?.[1];
      const rest = text.split('\n').slice(1).join(' ').trim() || /\)\.\s*(.+)$/s.exec(text.split('\n')[0])?.[1];
      return {
        tone: 'ok', title: `Gemergt${target ? ` in ${target}` : ''}`, highlight: true,
        meta: [who(e), ...(e.ref?.commit ? [`Commit ${e.ref.commit.slice(0, 9)}`] : [])], detail: rest || undefined,
      };
    }
    case 'dispatch:merge-conflict':
      return { tone: 'error', title: 'Merge-Konflikt', meta, highlight: true, error: afterMarker(text, '[dispatch] Merge-Konflikt') };
    case 'dispatch:merge-wait':
      return { tone: 'warn', title: 'Merge wartet', meta, detail: afterMarker(text, '[dispatch] Merge wartet'), highlight: false };
    case 'dispatch:merge': {
      const failed = text.includes('nicht möglich');
      return failed
        ? { tone: 'error', title: 'Merge nicht möglich', meta, highlight: true, error: afterMarker(text, '[dispatch] Merge nicht möglich') }
        : { tone: 'plain', title: afterMarker(text, '[dispatch] Merge') || 'Merge', meta, highlight: false };
    }
    case 'run-log':
    case 'run-end': {
      const label = Object.keys(RUN_LABEL).find((l) => text.startsWith(l)) ?? 'Lauf';
      const log = e.ref?.runLog ? [`Protokoll ${e.ref.runLog}`] : [];
      if (e.type === 'run-log') return { tone: 'plain', title: text || `${label} gestartet`, meta: [who(e), ...log], highlight: false };
      const ok = e.ref?.exitCode === 0;
      return { tone: ok ? 'ok' : 'error', title: `${label} beendet`, meta: [who(e), ...log], highlight: true, ...(ok ? {} : { error: text.replace(/^.*?beendet:\s*/, '') }) };
    }
    case 'commit':
      return { tone: 'plain', title: text, meta: [`Commit ${e.ref?.commit?.slice(0, 9) ?? ''}`, e.actor.name], highlight: false };
  }
  if (e.type.startsWith('question:')) {
    const kind = e.type.slice('question:'.length);
    const [head, ...rest] = text.split(': ');
    const titles: Record<string, string> = {
      created: 'Frage gestellt', research: 'Frage recherchiert', tickets: 'Betroffene Tickets der Frage geändert',
      decided: 'Frage entschieden', discarded: 'Frage verworfen', reopened: 'Frage wieder geöffnet',
    };
    return {
      tone: kind === 'decided' ? 'ok' : kind === 'reopened' || kind === 'created' ? 'warn' : 'plain',
      title: `${titles[kind] ?? 'Frage'}: ${head}`, meta, detail: rest.join(': ') || undefined,
      highlight: kind === 'decided' || kind === 'created', questionId: e.ref?.questionId,
    };
  }
  return { tone: 'plain', title: text || e.type, meta, highlight: false };
}

// Überschrift eines Tages: „Heute“, „Gestern“ oder „5. Oktober“ (mit Jahr, wenn nicht das laufende)
export function dayLabel(iso: string, now: Date = new Date()): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const day = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const diff = Math.round((day(now) - day(d)) / 86_400_000);
  if (diff === 0) return 'Heute';
  if (diff === 1) return 'Gestern';
  return d.toLocaleDateString('de-DE', { day: 'numeric', month: 'long', ...(d.getFullYear() === now.getFullYear() ? {} : { year: 'numeric' }) });
}

// Uhrzeit HH:MM (lokale Zeit)
export function timeLabel(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? '' : d.toLocaleTimeString('de-DE', { hour: '2-digit', minute: '2-digit' });
}
