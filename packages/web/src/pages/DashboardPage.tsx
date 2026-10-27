import { useState } from 'react';
import { useNavigate, useParams, useSearchParams } from 'react-router-dom';
import { useApi } from '../api/ApiContext.js';
import { ApiError } from '../api/ApiError.js';
import type { DispatchStatusDto } from '../api/dispatchTypes.js';
import type { DispatchRunKindDto, DispatchStatsDto } from '../api/statsTypes.js';
import { AbortReasonsCard } from '../components/dashboard/AbortReasonsCard.js';
import { KpiTile } from '../components/dashboard/KpiTile.js';
import { LadderCard } from '../components/dashboard/LadderCard.js';
import { ObstaclesCard } from '../components/dashboard/ObstaclesCard.js';
import { RunsChart } from '../components/dashboard/RunsChart.js';
import { Segmented } from '../components/dashboard/Segmented.js';
import { TodoColumn } from '../components/dashboard/TodoColumn.js';
import { DbUnavailable } from '../components/db/DbUnavailable.js';
import { DispatchDialog } from '../components/db/DispatchDialog.js';
import { DispatchIndicator } from '../components/db/DispatchIndicator.js';
import { EmptyState } from '../components/EmptyState.js';
import { ErrorState } from '../components/ErrorState.js';
import { LoadingState } from '../components/LoadingState.js';
import { formatDay, formatPercent, isoDay, plural } from '../dashboard/formatStats.js';
import { useBoardProjectSelection } from '../dbboard/boardProject.js';
import { useDispatchStatus } from '../dbboard/useDispatchStatus.js';
import { useAsync } from '../hooks/useAsync.js';

/** Lauf-Arten (Planungsläufe erfasst der Dispatcher nicht) */
const KINDS: Array<{ value: DispatchRunKindDto; label: string }> = [
  { value: 'work', label: 'Arbeit' }, { value: 'research', label: 'Recherche' }, { value: 'merge', label: 'Merge' },
];
type Period = '7' | '28' | '56' | 'cfg' | 'custom';
const PERIODS: Array<{ value: Period; label: string }> = [
  { value: '7', label: 'Letzte 7 Tage' }, { value: '28', label: 'Letzte 4 Wochen' }, { value: '56', label: 'Letzte 8 Wochen' },
  { value: 'cfg', label: 'Seit Konfig-Änderung' }, { value: 'custom', label: 'Benutzerdefiniert …' },
];
type BoardScope = 'current' | 'all';
const DAY = /^\d{4}-\d{2}-\d{2}$/;
/** Ab dieser Eskalationsquote erscheint der Wert orange */
const ESCALATION_WARN = 0.3;

/** from/to: erster und letzter Tag (JJJJ-MM-TT), nur beim Zeitraum „Benutzerdefiniert“ */
interface DashboardFilter { board: number | null; kind: DispatchRunKindDto; period: Period; from?: string; to?: string }

// Filter aus der Adresse; Standard: alle Boards, Arbeitsläufe, 4 Wochen. Benutzerdefiniert ohne gültige Tage: 4 Wochen
function filterFrom(search: URLSearchParams): DashboardFilter {
  const board = Number(search.get('board'));
  const kind = search.get('kind');
  const period = search.get('period');
  const from = search.get('from') ?? '';
  const to = search.get('to') ?? '';
  const custom = period === 'custom' && DAY.test(from) && DAY.test(to);
  return {
    board: Number.isInteger(board) && board > 0 ? board : null,
    kind: KINDS.some((k) => k.value === kind) ? kind as DispatchRunKindDto : 'work',
    period: custom ? 'custom' : PERIODS.some((p) => p.value === period && p.value !== 'custom') ? period as Period : '28',
    ...(custom ? (from <= to ? { from, to } : { from: to, to: from }) : {}),
  };
}

// Erster Tag eines festen Zeitraums (die letzten n Tage bis heute)
function startOf(today: Date, days: number): string {
  return isoDay(new Date(today.getFullYear(), today.getMonth(), today.getDate() - (days - 1)));
}

// Adresse aus Projekt und Filter
function query(project: string, f: DashboardFilter): URLSearchParams {
  const q = new URLSearchParams({ project });
  if (f.board) q.set('board', String(f.board));
  if (f.kind !== 'work') q.set('kind', f.kind);
  if (f.period !== '28') q.set('period', f.period);
  if (f.period === 'custom' && f.from && f.to) {
    q.set('from', f.from);
    q.set('to', f.to);
  }
  return q;
}

// Seite „Dashboard“ für das Board-Projekt der Seitenleiste. Links „Was kann ich optimieren?“ (Kennzahlen, Läufe je Tag,
// Eskalation entlang der Profile, Abbruchgründe, Hindernisse), rechts „Was muss ich tun?“ (Meine Aufgaben in Gruppen).
// Filter (Board, Lauf-Art, Zeitraum) stehen in der Adresse. Board: das aktuelle (aus der Seitenleiste) oder alle.
export function DashboardPage({ today = new Date() }: { today?: Date }) {
  const { pid = '' } = useParams();
  const api = useApi();
  const navigate = useNavigate();
  const [search, setSearch] = useSearchParams();
  const [metaUnavailable, setMetaUnavailable] = useState<string | null>(null);
  const meta = useAsync(async () => {
    try {
      const result = { projects: await api.getDbProjects(), active: await api.getDbActive(), boards: await api.getDbBoards() };
      setMetaUnavailable(null);
      return result;
    } catch (error) {
      if (error instanceof ApiError && error.status === 503) setMetaUnavailable(error.message);
      throw error;
    }
  }, [api]);
  // Leiter der Profile aus der Dispatcher-Einstellung; fehlt sie, folgen die Profile der Zahl der Läufe
  const config = useAsync(async () => (await api.getDispatchConfig().catch(() => null))?.config?.ladder ?? [], [api]);
  const ready = meta.state.status === 'ready' ? meta.state.data : null;
  const projects = ready?.projects ?? [];
  const requested = search.get('project');
  const projectKey = ready
    ? (requested && projects.some((p) => p.key === requested) ? requested : ready.active.board?.projectKey ?? projects[0]?.key ?? null)
    : null;
  const boards = (ready?.boards ?? []).filter((b) => b.projectKey === projectKey);
  const filter = filterFrom(search);
  // Aktuelles Board: das gefilterte, sonst das aktive Board, sofern es zum Projekt gehört
  const current = boards.find((b) => b.id === filter.board) ?? boards.find((b) => b.id === ready?.active.board?.id) ?? null;
  // „Was muss ich tun?“ folgt dem Board-Filter nur bei einem eigenen Board, sonst alle eigenen Boards
  const ownBoard = boards.some((b) => b.id === filter.board && b.ownerId === ready?.active.user.id) ? filter.board : null;
  // Die Board-Auswahl der Seitenleiste setzt Projekt und Board-Filter; Lauf-Art und Zeitraum bleiben
  useBoardProjectSelection(projectKey, (choice) => setSearch(query(choice.project, { ...filter, board: choice.board })), filter.board);

  // Zustand des Dispatchers wie auf dem Board: Knopf im Kopf öffnet die Übersicht
  const dispatch = useDispatchStatus(filter.board);
  const [dispatchOpen, setDispatchOpen] = useState(false);

  const [statsUnavailable, setStatsUnavailable] = useState<string | null>(null);
  const stats = useAsync(async (): Promise<DispatchStatsDto | null> => {
    if (!projectKey) return null;
    const custom = filter.period === 'custom' && filter.from && filter.to ? { from: filter.from, to: filter.to } : null;
    const from = custom?.from ?? (filter.period === 'cfg' ? 'config' : startOf(today, Number(filter.period)));
    try {
      const result = await api.getDispatchStats({ project: projectKey, board: filter.board, from, to: custom?.to ?? isoDay(today), kind: filter.kind });
      setStatsUnavailable(null);
      return result;
    } catch (error) {
      if (error instanceof ApiError && error.status === 503) setStatsUnavailable(error.message);
      throw error;
    }
  }, [api, projectKey, filter.board, filter.kind, filter.period, filter.from, filter.to]);
  // Zuletzt bekannter Stand der Einstellung (für den Zeitraum „Seit Konfig-Änderung“)
  const loaded = stats.state.status === 'ready' ? stats.state.data : null;

  const unavailable = metaUnavailable ?? statsUnavailable;
  if (unavailable) {
    return (
      <div className="page">
        <DbUnavailable pid={pid} message={unavailable} onRetry={() => { void meta.retry(); void stats.retry(); }} />
      </div>
    );
  }

  const change = (next: Partial<DashboardFilter>) => projectKey && setSearch(query(projectKey, { ...filter, ...next }));
  const changedAt = loaded?.configChangedAt ?? null;
  // „Benutzerdefiniert“ startet mit dem zuletzt geladenen Zeitraum
  const choosePeriod = (period: Period) => change(period === 'custom'
    ? { period, from: loaded?.filter.from ?? startOf(today, 28), to: loaded?.filter.to ?? isoDay(today) }
    : { period });
  // Liegt der Beginn nach dem Ende, zieht die andere Grenze mit
  const setDay = (edge: 'from' | 'to', day: string) => {
    if (!DAY.test(day) || !filter.from || !filter.to) return;
    const next = { from: filter.from, to: filter.to, [edge]: day };
    change(next.from <= next.to ? next : { from: day, to: day });
  };
  return (
    <div className="page dashboard" aria-label="Dashboard">
      <header className="dash-head">
        <div className="dash-title">
          <h1>Dashboard</h1>
        </div>
        {projectKey && (
          <>
            <Segmented<BoardScope>
              label="Board" value={filter.board ? 'current' : 'all'} onChange={(scope) => change({ board: scope === 'current' ? current?.id ?? null : null })}
              options={[
                { value: 'current', label: 'Aktuelles Board', disabled: !current, title: current ? `${current.name} (${current.ownerName})` : 'Kein Board in der Seitenleiste gewählt' },
                { value: 'all', label: 'Alle Boards' },
              ]}
            />
            <Segmented label="Lauf-Art" options={KINDS} value={filter.kind} onChange={(kind) => change({ kind })} />
            <select aria-label="Zeitraum" value={filter.period} onChange={(e) => choosePeriod(e.target.value as Period)}>
              {PERIODS.map((p) => (p.value === 'cfg'
                ? (
                  <option key={p.value} value={p.value} disabled={!changedAt && filter.period !== 'cfg'}>
                    {changedAt ? `Seit Konfig-Änderung (${formatDay(isoDay(new Date(changedAt)), true)})` : `${p.label} (noch keine)`}
                  </option>
                )
                : <option key={p.value} value={p.value}>{p.label}</option>))}
            </select>
            {filter.period === 'custom' && (
              <div className="dash-range" role="group" aria-label="Eigener Zeitraum">
                <input type="date" aria-label="Von" value={filter.from} max={isoDay(today)} onChange={(e) => setDay('from', e.target.value)} />
                <span aria-hidden="true">–</span>
                <input type="date" aria-label="Bis" value={filter.to} max={isoDay(today)} onChange={(e) => setDay('to', e.target.value)} />
              </div>
            )}
          </>
        )}
        <DispatchIndicator status={dispatch.status} error={dispatch.error} onOpen={() => setDispatchOpen(true)} />
      </header>
      {dispatchOpen && (
        <DispatchDialog
          boardId={filter.board}
          status={dispatch.status}
          error={dispatch.error}
          onReload={() => void dispatch.reload()}
          pid={pid}
          onClose={() => setDispatchOpen(false)}
          onStatus={dispatch.set}
          onOpenTicket={(id) => navigate(`/p/${encodeURIComponent(pid)}/board/t/${encodeURIComponent(id)}`)}
          onChanged={() => void stats.retry()}
        />
      )}
      {ready && !projects.length && <EmptyState title="Noch kein Board-Projekt">Ein Projekt legst du über den Stift neben der Board-Auswahl in der Seitenleiste an.</EmptyState>}
      {(meta.state.status === 'loading' || (projectKey && stats.state.status === 'loading')) && <LoadingState text="Auswertung wird geladen …" />}
      {meta.state.status === 'error' && <ErrorState message="Board-Projekte konnten nicht geladen werden" detail={meta.state.message} onRetry={meta.retry} />}
      {stats.state.status === 'error' && <ErrorState message="Auswertung konnte nicht geladen werden" detail={stats.state.message} onRetry={stats.retry} />}
      {loaded && (
        <Content
          pid={pid} data={loaded} ownBoard={ownBoard} today={today}
          ladder={config.state.status === 'ready' ? config.state.data : []}
          status={dispatch.status}
        />
      )}
    </div>
  );
}

// Kennzahlen, linke Spalte (Optimieren) und rechte Spalte (Aufgaben)
function Content({ pid, data, ownBoard, ladder, status, today }: {
  pid: string; data: DispatchStatsDto; ownBoard: number | null; ladder: string[]; status: DispatchStatusDto | null; today: Date;
}) {
  const { kpis } = data;
  const subjects = data.kind === 'research' ? ['Frage', 'Fragen'] as const : ['Ticket', 'Tickets'] as const;
  return (
    <>
      {(!data.runsSince || data.backfilledRuns > 0) && (
        <p className="dashboard-note" role="note">
          {!data.runsSince && 'Der Dispatcher hat in diesem Projekt noch keine Läufe erfasst; die Auswertung erscheint ab dem ersten erfassten Lauf.'}
          {data.backfilledRuns > 0 && ` Aus [dispatch]-Kommentaren rekonstruiert: ${plural(data.backfilledRuns, 'Lauf', 'Läufe')} im Zeitraum.`}
        </p>
      )}
      <div className="kpi-grid">
        <KpiTile label="Läufe" value={String(kpis.runs)} sub={`${plural(kpis.subjects, subjects[0], subjects[1])} im Zeitraum`} />
        <KpiTile label="Erfolgsquote" value={formatPercent(kpis.successRate)} sub={`${kpis.solved} gelöst · ${data.ladder.finished - kpis.solved} endgültig gescheitert`} />
        <KpiTile label="Erstversuch-Quote" value={formatPercent(kpis.firstTryRate)} sub="im 1. Lauf gelöst, ohne Eskalation" tone="good" />
        <KpiTile
          label="Eskalationsquote" value={formatPercent(kpis.escalationRate)} sub={`${subjects[1]} mit mind. einer Eskalation`}
          tone={(kpis.escalationRate ?? 0) > ESCALATION_WARN ? 'warn' : 'neutral'}
        />
        <KpiTile label="Endgültig gescheitert" value={String(kpis.finalFailures)} sub="Versuche aufgebraucht" tone={kpis.finalFailures ? 'bad' : 'neutral'} />
      </div>
      <div className="dash-main">
        <div className="dash-left">
          <section className="dash-card" aria-labelledby="dash-series">
            <div className="dash-card-head">
              <h2 id="dash-series">Läufe je {data.series.unit === 'day' ? 'Tag' : 'Woche'}</h2>
              <div className="chart-legend">
                <span><i className="dot done" aria-hidden="true" />erfolgreich</span>
                <span><i className="dot fail" aria-hidden="true" />abgebrochen</span>
                <span><i className="dot other" aria-hidden="true" />Frage / blockiert</span>
              </div>
            </div>
            <RunsChart series={data.series} />
          </section>
          <div className="dash-pair">
            <LadderCard data={data} ladder={data.kind === 'work' ? ladder : []} />
            <AbortReasonsCard data={data} />
          </div>
          <ObstaclesCard status={status} data={data} />
        </div>
        <TodoColumn pid={pid} boardId={ownBoard} now={today} />
      </div>
    </>
  );
}
