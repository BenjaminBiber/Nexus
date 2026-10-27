import { useId, useRef, useState, type KeyboardEvent } from 'react';
import { useParams, useSearchParams } from 'react-router-dom';
import { useApi } from '../api/ApiContext.js';
import { ApiError } from '../api/ApiError.js';
import type { DbQuestionDto, DbQuestionListDto } from '../api/dbTypes.js';
import { DbAskQuestionDialog } from '../components/db/DbAskQuestionDialog.js';
import { DbQuestionList } from '../components/db/DbQuestionList.js';
import { DbUnavailable } from '../components/db/DbUnavailable.js';
import { EmptyState } from '../components/EmptyState.js';
import { ErrorState } from '../components/ErrorState.js';
import { LoadingState } from '../components/LoadingState.js';
import { QuestionFilters } from '../components/QuestionFilters.js';
import { QuickCapture } from '../components/QuickCapture.js';
import { dbQuestionFacets, filterDbQuestions } from '../dbquestion/questionDtos.js';
import { useDbQuestions } from '../dbquestion/useDbQuestions.js';
import { useBoardProjectSelection } from '../dbboard/boardProject.js';
import { notifyDbActiveChanged } from '../dbquestion/activeBoard.js';
import { useAsync } from '../hooks/useAsync.js';
import { NO_FILTER, isFiltering, type QuestionFilterState } from '../question/filterQuestions.js';

// Reiter: offene eigene (Standard), offene andere, beantwortete, verworfene, alle
type StateFilter = 'mine' | 'others' | 'answered' | 'discarded' | 'all';

const FILTERS: StateFilter[] = ['mine', 'others', 'answered', 'discarded', 'all'];

const FILTER_LABELS: Record<StateFilter, string> = { mine: 'Eigene', others: 'Andere offene', answered: 'Beantwortet', discarded: 'Verworfen', all: 'Alle' };

// Zähler eines Reiters aus den Zählern des Servers; „Andere offene“ = offen ohne eigene, „Alle“ ist die Summe
function countOf(data: DbQuestionListDto, value: StateFilter): number {
  const { open, answered, discarded, mine } = data.counts;
  if (value === 'mine') return mine;
  if (value === 'others') return open - mine;
  return value === 'all' ? open + answered + discarded : data.counts[value];
}

// Gehört die Frage in den Reiter? „Eigene“ kennzeichnet der Server (mine)
function inFilter(q: DbQuestionDto, value: StateFilter): boolean {
  if (value === 'all') return true;
  if (value === 'mine') return q.state === 'open' && q.mine === true;
  if (value === 'others') return q.state === 'open' && q.mine !== true;
  return q.state === value;
}

// Seite „Fragen“: Fragen des Board-Projekts aus der Seitenleiste mit Zustandsreitern, Suche, Filtern wie auf der alten Fragenseite,
// Schnellanlage und „Neue Frage“; entschieden wird im Detail.
export function DbQuestionsPage() {
  const { pid = '' } = useParams();
  const api = useApi();
  const [search, setSearch] = useSearchParams();
  const [metaUnavailable, setMetaUnavailable] = useState<string | null>(null);
  // Projekte und aktives Board (für die Vorauswahl); 503 führt zum Hinweis auf die Einstellungen
  const meta = useAsync(async () => {
    try {
      const result = { projects: await api.getDbProjects(), active: await api.getDbActive() };
      setMetaUnavailable(null);
      return result;
    } catch (error) {
      if (error instanceof ApiError && error.status === 503) setMetaUnavailable(error.message);
      throw error;
    }
  }, [api]);
  const projects = meta.state.status === 'ready' ? meta.state.data.projects : [];
  const activeKey = meta.state.status === 'ready' ? meta.state.data.active.board?.projectKey ?? null : null;
  // Ein unbekanntes Projekt in der Adresse fällt auf das des aktiven Boards (sonst das erste) zurück
  const requested = search.get('project');
  const requestedKnown = requested !== null && projects.some((p) => p.key === requested);
  const fallbackKey = activeKey ?? projects[0]?.key ?? null;
  const projectKey = meta.state.status === 'ready' ? (requestedKnown ? requested : fallbackKey) : null;
  const unknownProject = meta.state.status === 'ready' && requested !== null && !requestedKnown && projects.length > 0 ? requested : null;
  const projectId = projects.find((p) => p.key === projectKey)?.id ?? null;
  const questions = useDbQuestions(projectKey, projectId);
  // Fragen gehören zum Projekt; ein gewähltes Board wird zusätzlich aktiv
  useBoardProjectSelection(projectKey, (choice) => {
    setSearch({ project: choice.project });
    if (choice.board !== null) void api.activateDbBoard(choice.board).then(notifyDbActiveChanged, () => meta.reload());
  });
  const [filter, setFilter] = useState<StateFilter>('mine');
  const [criteria, setCriteria] = useState<QuestionFilterState>(NO_FILTER);
  const [asking, setAsking] = useState(false);
  const [freshId, setFreshId] = useState<string | null>(null);
  const unavailable = metaUnavailable ?? questions.unavailable;
  const tabsId = useId();
  const tabRefs = useRef(new Map<StateFilter, HTMLButtonElement>());
  const tabId = (value: StateFilter) => `${tabsId}-tab-${value}`;
  const panelId = `${tabsId}-panel`;

  // Pfeiltasten wechseln den Reiter (ringsum), Pos1/Ende springen an den Rand
  const onTabKey = (event: KeyboardEvent<HTMLButtonElement>) => {
    const index = FILTERS.indexOf(filter);
    const next =
      event.key === 'ArrowRight' ? (index + 1) % FILTERS.length
        : event.key === 'ArrowLeft' ? (index - 1 + FILTERS.length) % FILTERS.length
          : event.key === 'Home' ? 0
            : event.key === 'End' ? FILTERS.length - 1
              : null;
    if (next === null) return;
    event.preventDefault();
    setFilter(FILTERS[next]);
    tabRefs.current.get(FILTERS[next])?.focus();
  };

  // Schnellanlage: nur der Titel, Anlass „spontan“ (Herkunft Mensch setzt der Server); danach hervorgehoben in „Eigene“
  const quick = async (title: string) => {
    if (!projectKey) return;
    const created = await api.createDbQuestion(projectKey, { title, occasion: 'spontan' });
    setFreshId(created.id);
    if (filter !== 'all') setFilter('mine');
    void questions.reload();
  };

  if (unavailable) {
    return (
      <div className="page">
        <DbUnavailable pid={pid} message={unavailable} onRetry={() => { void meta.retry(); void questions.retry(); }} />
      </div>
    );
  }

  // Liste, Reiter und Leerzustände für geladene Daten
  const content = (data: DbQuestionListDto) => {
    const inState = data.questions.filter((q) => inFilter(q, filter));
    const shown = filterDbQuestions(inState, 'all', criteria);
    const filtering = isFiltering(criteria);
    return (
      <>
        <div className="tabs state-tabs" role="tablist" aria-label="Zustand">
          {FILTERS.map((value) => {
            const label = FILTER_LABELS[value];
            const count = countOf(data, value);
            return (
              <button
                key={value}
                ref={(el) => {
                  if (el) tabRefs.current.set(value, el);
                  else tabRefs.current.delete(value);
                }}
                id={tabId(value)}
                type="button"
                role="tab"
                className={`tab${filter === value ? ' on' : ''}`}
                aria-label={`${label} (${count})`}
                aria-selected={filter === value}
                aria-controls={panelId}
                tabIndex={filter === value ? 0 : -1}
                onClick={() => setFilter(value)}
                onKeyDown={onTabKey}
              >
                {label}
                <span className="mono tab-count" aria-hidden="true">{count}</span>
              </button>
            );
          })}
        </div>
        <QuestionFilters questions={inState.map(dbQuestionFacets)} withSearch={false} value={criteria} onChange={setCriteria} />
        <div role="tabpanel" id={panelId} aria-labelledby={tabId(filter)}>
          {shown.length === 0 && filter === 'mine' && !filtering && (
            <EmptyState title="Keine eigenen offenen Fragen">
              Eigene Fragen hast du selbst gestellt oder sie betreffen Tickets auf deinen Boards. Fragen entstehen beim Planen oder Umsetzen
              eines Tickets oder über „Neue Frage“; offene Fragen anderer stehen unter „Andere offene“.
            </EmptyState>
          )}
          {shown.length === 0 && (filter !== 'mine' || filtering) && (
            <div className="state empty">
              <h2>Keine Fragen mit diesen Filtern</h2>
              {filtering && (
                <button type="button" className="btn" onClick={() => setCriteria(NO_FILTER)}>
                  Filter zurücksetzen
                </button>
              )}
            </div>
          )}
          {shown.length > 0 && <DbQuestionList pid={pid} questions={shown} freshId={freshId} />}
        </div>
      </>
    );
  };

  return (
    <div className="page" aria-label="Fragen">
      <header className="questions-head">
        <h1>Fragen</h1>
        <span className="muted">Entscheidungen, auf die Tickets warten. Entschieden wird von dir.</span>
      </header>
      <div className="db-questions-bar">
        <input type="search" className="db-search" aria-label="Suche" placeholder="Suchen (Nummer, Titel, Tag, Ticket)" value={criteria.text} onChange={(e) => setCriteria({ ...criteria, text: e.target.value })} />
        <button type="button" className="btn primary" disabled={!projectKey} onClick={() => setAsking(true)}>
          Neue Frage
        </button>
      </div>
      {projectKey && <QuickCapture key={projectKey} onCreate={quick} submitLabel="Anlegen" />}
      {unknownProject && (
        <p role="status" className="muted">
          Projekt „{unknownProject}“ gibt es nicht; gezeigt wird {projectKey}.
        </p>
      )}
      {questions.notice && questions.state.status === 'ready' && (
        <p role="alert" className="field-error">
          {questions.notice}
        </p>
      )}
      {meta.state.status === 'ready' && !projects.length && (
        <EmptyState title="Noch kein Board-Projekt">Ein Projekt legst du über den Stift neben der Board-Auswahl in der Seitenleiste an.</EmptyState>
      )}
      {(meta.state.status === 'loading' || questions.state.status === 'loading') && <LoadingState text="Fragen werden geladen …" />}
      {meta.state.status === 'error' && <ErrorState message="Board-Projekte konnten nicht geladen werden" detail={meta.state.message} onRetry={meta.retry} />}
      {questions.state.status === 'error' && <ErrorState message="Fragen konnten nicht geladen werden" detail={questions.state.message} onRetry={questions.retry} />}
      {questions.state.status === 'ready' && questions.state.data && content(questions.state.data)}
      {asking && projectKey && (
        <DbAskQuestionDialog
          projectKey={projectKey}
          onClose={() => setAsking(false)}
          onCreated={(created) => {
            setAsking(false);
            setFreshId(created.id);
            void questions.reload();
          }}
        />
      )}
    </div>
  );
}
