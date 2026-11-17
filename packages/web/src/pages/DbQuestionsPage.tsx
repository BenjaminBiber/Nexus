import { useEffect, useId, useRef, useState, type KeyboardEvent } from 'react';
import { useNavigate, useParams, useSearchParams } from 'react-router-dom';
import { useApi } from '../api/ApiContext.js';
import { ApiError } from '../api/ApiError.js';
import type { DbQuestionDetailDto, DbQuestionDto, DbQuestionListDto } from '../api/dbTypes.js';
import { DbAskQuestionDialog } from '../components/db/DbAskQuestionDialog.js';
import { DbQuestionDetail } from '../components/db/DbQuestionDetail.js';
import { DbQuestionList, questionGroups } from '../components/db/DbQuestionList.js';
import { DbUnavailable } from '../components/db/DbUnavailable.js';
import { EmptyState } from '../components/EmptyState.js';
import { ErrorState } from '../components/ErrorState.js';
import { LoadingState } from '../components/LoadingState.js';
import { QuestionFilters } from '../components/QuestionFilters.js';
import { useBoardProjectSelection } from '../dbboard/boardProject.js';
import { notifyDbActiveChanged } from '../dbquestion/activeBoard.js';
import { dbQuestionFacets, filterDbQuestions } from '../dbquestion/questionDtos.js';
import { QUESTION_ID } from '../dbquestion/questionFormat.js';
import { useDbQuestions } from '../dbquestion/useDbQuestions.js';
import { useAsync } from '../hooks/useAsync.js';
import { NO_FILTER, isFiltering, type QuestionFilterState } from '../question/filterQuestions.js';

// Reiter: offene eigene (Standard), offene andere, erledigte (beantwortet oder verworfen)
type Tab = 'mine' | 'others' | 'done';

const TABS: Tab[] = ['mine', 'others', 'done'];

const TAB_LABELS: Record<Tab, string> = { mine: 'Eigene', others: 'Andere', done: 'Erledigt' };

const LIST_KEY = 'nexus.questions.list';
const RAIL_KEY = 'nexus.questions.rail';

// Zähler eines Reiters aus den Zählern des Servers; „Andere“ = offen ohne eigene
function countOf(data: DbQuestionListDto, tab: Tab): number {
  const { open, answered, discarded, mine } = data.counts;
  if (tab === 'mine') return mine;
  return tab === 'others' ? open - mine : answered + discarded;
}

// Reiter einer Frage; „Eigene“ kennzeichnet der Server (mine)
function tabOf(q: DbQuestionDto): Tab {
  if (q.state !== 'open') return 'done';
  return q.mine === true ? 'mine' : 'others';
}

// Liste und Detail stehen nebeneinander (breites Fenster); ohne matchMedia (Tests) gilt breit
function sideBySide(): boolean {
  return typeof window.matchMedia !== 'function' || !window.matchMedia('(max-width: 900px)').matches;
}

// Ein- oder ausgeklappter Bereich, im Browser gemerkt (ohne Speicher: ausgeklappt)
function useStoredOpen(key: string): [boolean, () => void] {
  const [open, setOpen] = useState(() => {
    try {
      return window.localStorage.getItem(key) !== '0';
    } catch {
      return true;
    }
  });
  const toggle = () => {
    setOpen(!open);
    try {
      window.localStorage.setItem(key, open ? '0' : '1');
    } catch {
      /* ohne Speicher gilt die Wahl nur bis zum Neuladen */
    }
  };
  return [open, toggle];
}

// Seite „Fragen“: links die Fragen des Board-Projekts (Reiter Eigene/Andere/Erledigt, Suche, Filter), daneben die gewählte Frage
// zum Lesen und Entscheiden und rechts ihre Bezüge. Die gewählte Frage steht in der Adresse (/questions/ARC-F0001).
export function DbQuestionsPage() {
  const { pid = '', id } = useParams();
  const selected = id ?? null;
  const api = useApi();
  const navigate = useNavigate();
  const [search] = useSearchParams();
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
  const metaData = meta.state.status === 'ready' ? meta.state.data : null;
  const ready = metaData !== null;
  const projects = metaData?.projects ?? [];
  const activeKey = metaData?.active.board?.projectKey ?? null;
  // Projekt: das der gewählten Frage, sonst das aus der Adresse, sonst das des aktiven Boards (oder das erste)
  const idPrefix = selected ? QUESTION_ID.exec(selected)?.[1]?.toUpperCase() ?? null : null;
  const idProject = idPrefix !== null ? projects.find((p) => p.key.toUpperCase() === idPrefix)?.key ?? null : null;
  const requested = search.get('project');
  const requestedKnown = requested !== null && projects.some((p) => p.key === requested);
  const fallbackKey = activeKey ?? projects[0]?.key ?? null;
  const projectKey = ready ? idProject ?? (requestedKnown ? requested : fallbackKey) : null;
  const unknownProject = ready && !idProject && requested !== null && !requestedKnown && projects.length > 0 ? requested : null;
  const projectId = projects.find((p) => p.key === projectKey)?.id ?? null;
  const questions = useDbQuestions(projectKey, projectId);
  const listHref = (questionId?: string) =>
    `/p/${pid}/questions${questionId ? `/${questionId}` : projectKey ? `?project=${encodeURIComponent(projectKey)}` : ''}`;
  // Fragen gehören zum Projekt; ein gewähltes Board wird zusätzlich aktiv. Eine gewählte Frage eines anderen Projekts entfällt
  useBoardProjectSelection(projectKey, (choice) => {
    navigate(`/p/${pid}/questions?project=${encodeURIComponent(choice.project)}`);
    if (choice.board !== null) void api.activateDbBoard(choice.board).then(notifyDbActiveChanged, () => meta.reload());
  });
  const [tab, setTab] = useState<Tab>('mine');
  const [criteria, setCriteria] = useState<QuestionFilterState>(NO_FILTER);
  const [filtersOpen, setFiltersOpen] = useState(false);
  const [asking, setAsking] = useState(false);
  const [freshId, setFreshId] = useState<string | null>(null);
  const [listOpen, toggleList] = useStoredOpen(LIST_KEY);
  const [railOpen, toggleRail] = useStoredOpen(RAIL_KEY);
  const unavailable = metaUnavailable ?? questions.unavailable;
  const baseId = useId();
  const tabRefs = useRef(new Map<Tab, HTMLButtonElement>());
  const tabId = (value: Tab) => `${baseId}-tab-${value}`;
  const panelId = `${baseId}-panel`;
  const filterId = `${baseId}-filters`;
  const list = questions.state.status === 'ready' ? questions.state.data : null;

  // Die gewählte Frage steht im passenden Reiter (einmal je Auswahl, danach wählt der Benutzer)
  const synced = useRef<string | null>(null);
  useEffect(() => {
    if (!selected || !list || synced.current === selected) return;
    const found = list.questions.find((q) => q.id === selected);
    if (!found) return;
    synced.current = selected;
    setTab(tabOf(found));
  }, [selected, list]);

  // Fragen eines Reiters in Listenreihenfolge (gefiltert)
  const orderOf = (value: Tab) =>
    questionGroups(filterDbQuestions(list ? list.questions.filter((q) => tabOf(q) === value) : [], 'all', criteria)).flatMap((g) => g.items);
  const inTab = list ? list.questions.filter((q) => tabOf(q) === tab) : [];
  const shown = filterDbQuestions(inTab, 'all', criteria);
  const filtering = isFiltering(criteria);
  const facetFiltering = isFiltering({ ...criteria, text: '' });
  const order = questionGroups(shown).flatMap((g) => g.items);
  // Nur aus der Liste des aktuellen Projekts (nach einem Projektwechsel steht kurz noch die alte Liste)
  const firstId = order[0] && order[0].projectKey === projectKey ? order[0].id : null;

  // Ohne Auswahl ist die erste Frage gewählt – nur wenn Liste und Detail nebeneinander stehen; schmal zeigt „Alle Fragen“ die Liste.
  // Bei unbekanntem Projekt bleibt die Adresse, sonst verschwände der Hinweis darauf sofort.
  useEffect(() => {
    if (selected || !firstId || unknownProject || !sideBySide()) return;
    navigate(listHref(firstId), { replace: true });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selected, firstId]);

  // Reiterwechsel wählt dessen erste Frage (die bisherige stünde sonst in einem anderen Reiter)
  const chooseTab = (value: Tab) => {
    setTab(value);
    const first = orderOf(value)[0];
    if (first && first.id !== selected && sideBySide()) {
      synced.current = first.id;
      navigate(listHref(first.id));
    }
  };

  // Pfeiltasten wechseln den Reiter (ringsum), Pos1/Ende springen an den Rand
  const onTabKey = (event: KeyboardEvent<HTMLButtonElement>) => {
    const index = TABS.indexOf(tab);
    const next =
      event.key === 'ArrowRight' ? (index + 1) % TABS.length
        : event.key === 'ArrowLeft' ? (index - 1 + TABS.length) % TABS.length
          : event.key === 'Home' ? 0
            : event.key === 'End' ? TABS.length - 1
              : null;
    if (next === null) return;
    event.preventDefault();
    chooseTab(TABS[next]);
    tabRefs.current.get(TABS[next])?.focus();
  };

  // Neue Frage: in der Liste hervorheben und gleich öffnen
  const created = (question: DbQuestionDetailDto) => {
    setAsking(false);
    setFreshId(question.id);
    void questions.reload();
    navigate(listHref(question.id));
  };

  if (unavailable) {
    return (
      <div className="page">
        <DbUnavailable pid={pid} message={unavailable} onRetry={() => { void meta.retry(); void questions.retry(); }} />
      </div>
    );
  }

  const at = order.findIndex((q) => q.id === selected);
  const next = at >= 0 ? order[at + 1] ?? (order.length > 1 ? order[0] : null) : order[0] ?? null;
  const followUpId = selected && list ? list.questions.find((q) => q.occasion === `Folgefrage zu ${selected}`)?.id ?? null : null;
  const loading = meta.state.status === 'loading' || questions.state.status === 'loading';
  const noProjects = ready && projects.length === 0;

  // Inhalt der Listenspalte: Laden, Fehler, Leerzustände oder die gruppierte Liste
  const listBody = () => {
    if (loading) return <LoadingState text="Fragen werden geladen …" />;
    if (meta.state.status === 'error') return <ErrorState message="Board-Projekte konnten nicht geladen werden" detail={meta.state.message} onRetry={meta.retry} />;
    if (questions.state.status === 'error') return <ErrorState message="Fragen konnten nicht geladen werden" detail={questions.state.message} onRetry={questions.retry} />;
    if (shown.length > 0) return <DbQuestionList pid={pid} questions={shown} currentId={selected} freshId={freshId} />;
    if (!filtering && tab === 'mine') {
      return (
        <div className="qx-list-empty">
          <strong>Keine eigenen offenen Fragen</strong>
          <p>Eigene Fragen hast du selbst gestellt, oder sie betreffen Tickets auf deinen Boards. Offene Fragen anderer stehen unter „Andere“.</p>
        </div>
      );
    }
    return (
      <div className="qx-list-empty">
        <strong>{filtering ? 'Keine Fragen mit diesen Filtern' : tab === 'others' ? 'Keine offenen Fragen anderer' : 'Noch nichts erledigt'}</strong>
        {filtering && (
          <button type="button" className="qx-btn" onClick={() => setCriteria(NO_FILTER)}>
            Filter zurücksetzen
          </button>
        )}
      </div>
    );
  };

  return (
    <div className={`qx${selected ? ' has-selection' : ''}`}>
      <header className="qx-head">
        <h1>Fragen</h1>
        {list && (
          <div className="qx-seg" role="tablist" aria-label="Zustand">
            {TABS.map((value) => {
              const count = countOf(list, value);
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
                  className="qx-seg-tab"
                  aria-label={`${TAB_LABELS[value]} (${count})`}
                  aria-selected={tab === value}
                  aria-controls={panelId}
                  tabIndex={tab === value ? 0 : -1}
                  onClick={() => chooseTab(value)}
                  onKeyDown={onTabKey}
                >
                  {TAB_LABELS[value]}
                  {count > 0 && (
                    <span className="qx-seg-count" aria-hidden="true">
                      {count}
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        )}
        <div className="qx-head-tools">
          <div className="qx-search">
            <span className="material-icons" aria-hidden="true">
              search
            </span>
            <input
              type="search"
              aria-label="Fragen durchsuchen"
              placeholder="Fragen durchsuchen"
              value={criteria.text}
              onChange={(event) => setCriteria({ ...criteria, text: event.target.value })}
            />
          </div>
          <button
            type="button"
            className={`qx-btn icon${filtersOpen ? ' on' : ''}`}
            aria-label="Filter"
            title="Filter"
            aria-expanded={filtersOpen}
            aria-controls={filterId}
            onClick={() => setFiltersOpen(!filtersOpen)}
          >
            <span className="material-icons" aria-hidden="true">
              filter_list
            </span>
            {facetFiltering && <span className="qx-dot" aria-hidden="true" />}
          </button>
          <button type="button" className="qx-btn accent" disabled={!projectKey} onClick={() => setAsking(true)}>
            <span className="material-icons" aria-hidden="true">
              add
            </span>
            Neue Frage
          </button>
        </div>
      </header>
      {filtersOpen && (
        <div id={filterId} className="qx-filterbar">
          <QuestionFilters questions={inTab.map(dbQuestionFacets)} withSearch={false} value={criteria} onChange={setCriteria} />
          {filtering && (
            <button type="button" className="qx-btn ghost" onClick={() => setCriteria(NO_FILTER)}>
              Filter zurücksetzen
            </button>
          )}
        </div>
      )}
      {unknownProject && (
        <p role="status" className="qx-note">
          Projekt „{unknownProject}“ gibt es nicht; gezeigt wird {projectKey}.
        </p>
      )}
      {questions.notice && list && (
        <p role="alert" className="qx-note error">
          {questions.notice}
        </p>
      )}
      {noProjects ? (
        <div className="qx-body single">
          <EmptyState title="Noch kein Board-Projekt">Ein Projekt legst du über den Stift neben der Board-Auswahl in der Seitenleiste an.</EmptyState>
        </div>
      ) : (
        <div className={`qx-body${listOpen ? '' : ' list-closed'}${railOpen ? '' : ' rail-closed'}`}>
          <section className={`qx-list${listOpen ? '' : ' closed'}`} aria-label="Fragenliste">
            <div className="qx-list-body" role="tabpanel" id={panelId} aria-labelledby={list ? tabId(tab) : undefined}>
              {listBody()}
            </div>
            {listOpen ? (
              <button type="button" className="qx-collapse" aria-expanded="true" onClick={toggleList}>
                <span className="material-icons" aria-hidden="true">
                  chevron_left
                </span>
                Fragenliste einklappen
              </button>
            ) : (
              <button
                type="button"
                className="qx-collapsed"
                aria-expanded="false"
                aria-label={`Fragenliste ausklappen – ${shown.length === 1 ? '1 Frage' : `${shown.length} Fragen`}`}
                title="Fragenliste ausklappen"
                onClick={toggleList}
              >
                <span className="material-icons" aria-hidden="true">
                  chevron_right
                </span>
                <span className="qx-vertical">Fragenliste</span>
                <span className="qx-rule" aria-hidden="true" />
                <span className="qx-count" aria-hidden="true">
                  {shown.length}
                </span>
              </button>
            )}
          </section>
          {selected ? (
            <DbQuestionDetail
              key={selected}
              pid={pid}
              id={selected}
              listHref={listHref()}
              followUpId={followUpId}
              onChanged={() => void questions.reload()}
              onSkip={next ? () => navigate(listHref(next.id)) : null}
              onCreated={created}
              railOpen={railOpen}
              onToggleRail={toggleRail}
            />
          ) : (
            <div className="qx-scroll qx-none">
              <div className="qx-pick">
                <span className="material-icons" aria-hidden="true">
                  forum
                </span>
                <h2>Keine Frage gewählt</h2>
                <p>Wähle links eine Frage, um sie zu lesen und zu entscheiden.</p>
              </div>
            </div>
          )}
        </div>
      )}
      {asking && projectKey && <DbAskQuestionDialog projectKey={projectKey} onClose={() => setAsking(false)} onCreated={created} />}
    </div>
  );
}
