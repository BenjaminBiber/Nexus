import { useCallback, useEffect, useState } from 'react';
import { useApi } from '../api/ApiContext.js';
import type { ApiError } from '../api/ApiError.js';
import type { DocCommentDto, DocFileDto, DocsSource, DocCommentPatch, DocTicketRefDto, NewDocCommentInput, ReviseDocInput, ReviseDocResultDto, SearchHitDto, WriteDocSectionInput } from '../api/types.js';
import { useLiveHandler } from '../live/useLive.js';
import { useAsync } from './useAsync.js';

// Liste der Dokumente eines Projekts samt Quelle; lädt bei der Live-Meldung „docs-changed“ und nach einem Neuverbinden nach
export function useDocList(pid: string) {
  const api = useApi();
  const loaded = useAsync(() => api.getDocs(pid), [api, pid]);
  const { reload } = loaded;
  useLiveHandler({ onDocsChanged: () => void reload(), onReset: () => void reload() });
  return loaded;
}

// Dokumente fremder Boards (Board-Umschalter der Docs-Seite), je Board mit seiner Quelle. Die Dateien tragen `board` und
// `boardName`, damit Explorer und Viewer sie dem richtigen Repo zuordnen. Ein Board, das nicht lädt, liefert keine Dateien.
export interface BoardDocList {
  boardId: number;
  source: DocsSource | null;
  docs: DocFileDto[];
}

export function useBoardDocLists(pid: string, boards: number[]) {
  const api = useApi();
  const loaded = useAsync(
    () => Promise.all(boards.map((id) => api.getDocs(pid, id).then(
      (list): BoardDocList => ({
        boardId: id,
        source: list.source,
        docs: list.docs.map((d) => ({ ...d, board: id, boardName: list.source.boardName ?? undefined })),
      }),
      (): BoardDocList => ({ boardId: id, source: null, docs: [] }),
    ))),
    [api, pid, boards.join(',')],
  );
  const { reload } = loaded;
  useLiveHandler({ onDocsChanged: () => void reload(), onReset: () => void reload() });
  return loaded;
}

// Inhalt eines Dokuments; ohne Pfad oder bei fehlender Datei wird nichts geladen
export function useDocContent(pid: string, path: string | null, board?: number) {
  const api = useApi();
  return useAsync(() => (path ? api.getDoc(pid, path, board) : new Promise<never>(() => undefined)), [api, pid, path, board]);
}

// Suche mit kurzer Verzögerung; unter zwei Zeichen wird nicht gesucht. `boards` nennt die Quellen (undefined = Board des Ordners),
// Treffer aus fremden Boards tragen deren ID
export function useDocSearch(pid: string, query: string, boards: Array<number | undefined> = [undefined]): SearchHitDto[] | null {
  const api = useApi();
  const [hits, setHits] = useState<SearchHitDto[] | null>(null);
  useEffect(() => {
    const term = query.trim();
    if (term.length < 2) {
      setHits(null);
      return;
    }
    let cancelled = false;
    const timer = setTimeout(() => {
      Promise.all(boards.map((board) => api.searchDocs(pid, term, board).then((found) => found.map((hit) => (board === undefined ? hit : { ...hit, board }))))).then(
        (found) => !cancelled && setHits(found.flat()),
        () => !cancelled && setHits([]),
      );
    }, 250);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [api, pid, query, boards.join(',')]); // eslint-disable-line react-hooks/exhaustive-deps
  return hits;
}

// Speichert einen Abschnitt des Dokuments; ein 409 (doc-changed) wird getrennt gemeldet,
// damit der Editor offen bleiben und den eigenen Text behalten kann
export function useDocSection(pid: string, path: string | null, board?: number) {
  const api = useApi();
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [conflict, setConflict] = useState(false);

  // Verwirft Fehler und Konflikt beim Wechsel des Dokuments
  useEffect(() => {
    setError(null);
    setConflict(false);
  }, [pid, path]);

  const save = async (input: Omit<WriteDocSectionInput, 'path'>): Promise<boolean> => {
    if (!path) return false;
    setSaving(true);
    try {
      await api.writeDocSection(pid, { path, ...input }, board);
      setError(null);
      setConflict(false);
      return true;
    } catch (e) {
      const err = e as ApiError;
      setConflict(err.code === 'doc-changed' || err.status === 409);
      setError((e as Error).message);
      return false;
    } finally {
      setSaving(false);
    }
  };

  return { save, saving, error, conflict, clearConflict: () => { setConflict(false); setError(null); } };
}

// Lässt einen Bereich von Claude umschreiben; der Server schreibt nichts, das Übernehmen läuft
// über writeDocSection mit dem Hash aus der revise-Antwort (sonst kollidiert die eigene Änderung mit sich selbst).
export function useDocRevision(pid: string, path: string | null, board?: number) {
  const api = useApi();
  const [running, setRunning] = useState(false);
  const [saving, setSaving] = useState(false);
  const [result, setResult] = useState<ReviseDocResultDto | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Verwirft Lauf und Vorschlag beim Wechsel des Dokuments
  useEffect(() => {
    setResult(null);
    setError(null);
  }, [pid, path]);

  const reset = () => { setResult(null); setError(null); setRunning(false); };

  // Holt den Vorschlag; ein Fehler lässt den Dialog in der Anweisung stehen
  const request = async (input: Omit<ReviseDocInput, 'path'>): Promise<void> => {
    if (!path) return;
    setRunning(true);
    setError(null);
    try {
      setResult(await api.reviseDoc(pid, { path, ...input }, board));
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setRunning(false);
    }
  };

  // Schreibt den (womöglich von Hand geänderten) Vorschlag; false hält den Dialog offen
  const apply = async (content: string): Promise<boolean> => {
    if (!path || !result) return false;
    setSaving(true);
    try {
      await api.writeDocSection(pid, { path, startLine: result.startLine, endLine: result.endLine, content, expectedHash: result.hash }, board);
      setError(null);
      return true;
    } catch (e) {
      setError((e as Error).message);
      return false;
    } finally {
      setSaving(false);
    }
  };

  return { request, apply, reset, running, saving, result, error };
}

const NO_TICKETS: DocTicketRefDto[] = [];

// Tickets, die aus diesem Dokument entstanden sind. Ohne Pfad wird nichts geladen; ein Fehler ergibt
// eine leere Liste, denn die Chips sind Beiwerk und sollen das Dokument nicht als fehlerhaft zeigen.
// Nur lesend – das Anlegen neuer Tickets macht `useDocTickets` (hooks/useDocTickets.ts); danach lädt `reload` die Liste nach.
export function useDocTicketRefs(pid: string, path: string | null, board?: number): { refs: DocTicketRefDto[]; reload: () => void } {
  const api = useApi();
  const loaded = useAsync(
    () => (path ? api.getDocTickets(pid, path, board).catch(() => NO_TICKETS) : new Promise<never>(() => undefined)),
    [api, pid, path, board],
  );
  const { reload } = loaded;
  return { refs: loaded.state.status === 'ready' ? loaded.state.data : NO_TICKETS, reload: useCallback(() => void reload(), [reload]) };
}

const NO_COMMENTS: DocCommentDto[] = [];

// Kommentare eines Dokuments mit Anlegen, Ändern und Löschen; Fehler werden als Text gemeldet.
// Ohne Pfad (fehlende Datei) wird nichts geladen; Änderungen melden per Rückgabewert, ob sie gelungen sind.
export function useDocComments(pid: string, path: string | null, board?: number) {
  const api = useApi();
  const loaded = useAsync(
    () => (path ? api.getDocComments(pid, path, board) : new Promise<never>(() => undefined)),
    [api, pid, path, board],
  );
  const [mutationError, setError] = useState<string | null>(null);
  const comments: DocCommentDto[] = loaded.state.status === 'ready' ? loaded.state.data : NO_COMMENTS;

  // Verwirft den Änderungsfehler beim Wechsel des Dokuments
  useEffect(() => setError(null), [pid, path]);

  const error = mutationError ?? (loaded.state.status === 'error' ? loaded.state.message : null);

  // Führt eine Änderung aus und lädt danach neu
  const run = async (action: (path: string) => Promise<unknown>): Promise<boolean> => {
    if (!path) return false;
    try {
      await action(path);
      setError(null);
      loaded.reload();
      return true;
    } catch (e) {
      setError((e as Error).message);
      return false;
    }
  };

  return {
    comments,
    error,
    add: (input: NewDocCommentInput) => run((p) => api.addDocComment(pid, p, input, board)),
    update: (id: string, patch: DocCommentPatch) => run((p) => api.patchDocComment(pid, p, id, patch, board)),
    remove: (id: string) => run((p) => api.deleteDocComment(pid, p, id, board)),
  };
}
