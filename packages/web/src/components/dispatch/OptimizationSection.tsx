import { useCallback, useEffect, useState } from 'react';
import { useApi } from '../../api/ApiContext.js';
import { ApiError } from '../../api/ApiError.js';
import type { DispatchConfigViewDto, SuggestionDto, SuggestionListDto } from '../../api/dispatchTypes.js';
import { describeOp, SuggestionCard } from './SuggestionCard.js';

const count = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;
const money = (usd: number) => `${usd.toFixed(2).replace('.', ',')} $`;
/** Sekunden als mm:ss */
const clock = (s: number) => `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;

// Ergebnis-Meldung einer Claude-Analyse: Anzahl, Aufteilung, Kosten und unbrauchbare Antworten
function analysisNotice(list: SuggestionListDto): string {
  const a = list.analysis;
  const claude = [...list.suggestions, ...list.dismissed].filter((s) => s.source === 'claude');
  const parts: string[] = [];
  if (a && a.count > 0) {
    const recommendations = claude.filter((s) => s.patch).length;
    const hints = a.count - recommendations;
    parts.push(`Claude: ${count(a.count, 'Vorschlag', 'Vorschläge')} (${count(recommendations, 'Empfehlung', 'Empfehlungen')}, ${count(hints, 'Hinweis', 'Hinweise')})`);
  } else {
    parts.push('Claude sieht keine Verbesserung – die Einstellung passt zu den Zahlen.');
  }
  if (a && a.costUsd !== null) parts.push(money(a.costUsd));
  if (a && a.skipped > 0) parts.push(`${count(a.skipped, 'Antwort', 'Antworten')} unbrauchbar`);
  return parts.join(' · ');
}

interface OptimizationSectionProps {
  /** Formular des Panels hat ungespeicherte Änderungen oder speichert gerade */
  locked: boolean;
  /** Nach dem Übernehmen: neue Einstellung fürs Formular */
  onApplied: (view: DispatchConfigViewDto) => void;
}

// Optimierungs-Vorschläge aus den Läufen aller Projekte: übernehmen, ausblenden, Claude-Analyse anstoßen.
export function OptimizationSection({ locked, onApplied }: OptimizationSectionProps) {
  const api = useApi();
  const [list, setList] = useState<SuggestionListDto | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [analyzing, setAnalyzing] = useState(false);
  const [showDismissed, setShowDismissed] = useState(false);
  const [seconds, setSeconds] = useState(0);

  // Läuft eine Analyse, zählt die Anzeige im Sekundentakt mit; am Ende und beim Verlassen wieder abräumen
  useEffect(() => {
    if (!analyzing) return;
    setSeconds(0);
    const timer = setInterval(() => setSeconds((s) => s + 1), 1000);
    return () => clearInterval(timer);
  }, [analyzing]);

  const load = useCallback(async () => {
    try {
      setList(await api.getDispatchSuggestions());
      setError(null);
    } catch (e) {
      setError((e as Error).message);
    }
  }, [api]);
  useEffect(() => void load(), [load]);

  // Führt eine Aktion aus; veraltete Liste (Konflikt, Vorschlag weg) → neu laden und Bescheid sagen
  const act = async (action: () => Promise<SuggestionListDto>, done?: string | ((list: SuggestionListDto) => string)) => {
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      const next = await action();
      setList(next);
      if (done) setNotice(typeof done === 'function' ? done(next) : done);
    } catch (e) {
      if (e instanceof ApiError && (e.code === 'conflict' || e.code === 'suggestion-not-found')) {
        setNotice(e.message);
        await load();
      } else {
        setError((e as Error).message);
      }
    } finally {
      setBusy(false);
    }
  };

  const apply = (s: SuggestionDto) => act(async () => {
    const result = await api.applyDispatchSuggestion(s.id, list!.configHash);
    onApplied(result.view);
    return result.list;
  }, `Übernommen: ${(s.patch ?? []).map(describeOp).join(', ')}`);

  const analyze = async () => {
    setAnalyzing(true);
    try {
      await act(() => api.analyzeDispatchSuggestions(), analysisNotice);
    } finally {
      setAnalyzing(false);
    }
  };

  const card = (s: SuggestionDto, dismissed = false) => (
    <SuggestionCard key={s.id} suggestion={s} locked={locked} busy={busy} dismissed={dismissed}
      onApply={() => void apply(s)}
      onDismiss={() => void act(() => api.dismissDispatchSuggestion(s.id))}
      onRestore={() => void act(() => api.restoreDispatchSuggestion(s.id))} />
  );
  const recommended = list?.suggestions.filter((s) => s.patch) ?? [];
  const hints = list?.suggestions.filter((s) => !s.patch) ?? [];

  return (
    <div className="optimization">
      <div className="optimization-head">
        <h3>Optimierungen</h3>
        <button type="button" className="btn" disabled={analyzing || busy || !list} onClick={() => void analyze()}>
          {analyzing ? 'Claude analysiert …' : 'Tiefer analysieren'}
        </button>
      </div>
      <p className="muted">
        Claude liest die Kennzahlen unten und schlägt bis zu 5 Verbesserungen vor; der Lauf dauert meist unter einer Minute,
        höchstens 5, und kostet wenige Cent.
      </p>
      {analyzing && (
        <p className="optimization-notice" role="status">Claude analysiert … ({clock(seconds)} von max. 5:00)</p>
      )}
      {list && (
        <p className="muted">
          Basis: {count(list.basis.runs, 'Lauf', 'Läufe')} aus {count(list.basis.projects, 'Projekt', 'Projekten')} · je Profil seit seiner letzten Änderung
        </p>
      )}
      {error && <div className="banner" role="alert">{error}</div>}
      {notice && <p className="optimization-notice" aria-live="polite">{notice}</p>}
      {!list && !error && <p className="muted">Wird ausgewertet …</p>}
      {list && list.suggestions.length === 0 && <p className="muted">Keine Auffälligkeiten – der Dispatcher läuft rund.</p>}
      {recommended.length > 0 && (
        <section aria-label="Empfehlungen">
          <h4 className="optimization-group">Empfehlungen</h4>
          {recommended.map((s) => card(s))}
        </section>
      )}
      {hints.length > 0 && (
        <section aria-label="Hinweise">
          <h4 className="optimization-group">Hinweise</h4>
          {hints.map((s) => card(s))}
        </section>
      )}
      {list && (
        <div className="optimization-foot muted">
          {list.insufficient.length > 0 && (
            <p>Zu wenig Daten: {list.insufficient.map((i) => `${i.profile} (${i.runs}/${i.needed})`).join(', ')}</p>
          )}
          {list.dismissed.length > 0 && (
            <button type="button" className="btn" onClick={() => setShowDismissed((v) => !v)}>
              {list.dismissed.length} ausgeblendet · {showDismissed ? 'verbergen' : 'anzeigen'}
            </button>
          )}
          {showDismissed && list.dismissed.map((s) => card(s, true))}
          {list.analysis && (
            <p>
              Letzte Claude-Analyse: {new Date(list.analysis.at).toLocaleString('de-DE', { dateStyle: 'short', timeStyle: 'short' })}
              {` · ${count(list.analysis.count, 'Vorschlag', 'Vorschläge')}`}
              {list.analysis.costUsd !== null ? ` · ${money(list.analysis.costUsd)}` : ''}
              {list.analysis.skipped > 0 ? ` · ${count(list.analysis.skipped, 'Antwort', 'Antworten')} unbrauchbar` : ''}
              {list.analysis.stale ? ' · veraltet' : ''}
            </p>
          )}
        </div>
      )}
    </div>
  );
}
