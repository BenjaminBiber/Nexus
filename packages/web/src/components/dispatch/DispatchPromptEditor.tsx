import { useLayoutEffect, useRef, type ReactNode } from 'react';
import type { DispatchPromptPlaceholderDto } from '../../api/dispatchTypes.js';
import { Badge } from '../Badge.js';

/** Was der Server an einer Vorlage bemängelt hat (400 beim Speichern) */
export interface PromptProblem {
  missing: string[];
  unknown: string[];
}

export interface DispatchPromptEditorProps {
  label: string;
  /** Leerer Text = Standardvorlage */
  value: string;
  /** Standardvorlage des Servers; fehlt sie (älterer Server), entfällt „Standard übernehmen“ */
  defaultTemplate?: string;
  placeholders?: DispatchPromptPlaceholderDto[];
  /** Beschriftungen der Pflichtteile dieser Laufart */
  required?: string[];
  problem?: PromptProblem | null;
  onChange: (value: string) => void;
  /** Platz für die Vorschau (AIH-0091) */
  children?: ReactNode;
}

// Ein Editor für die Vorlage einer Laufart – ohne eigenen Zustand; Wert und Rückrufe kommen von oben.
export function DispatchPromptEditor({ label, value, defaultTemplate, placeholders = [], required = [], problem, onChange, children }: DispatchPromptEditorProps) {
  const area = useRef<HTMLTextAreaElement>(null);
  const caret = useRef<number | null>(null);
  const custom = value.trim() !== '';

  // React schreibt den neuen Wert erst beim Rendern; der Cursor wird danach gesetzt, sonst stünde er am Ende
  useLayoutEffect(() => {
    if (caret.current === null) return;
    const at = caret.current;
    caret.current = null;
    area.current?.focus();
    area.current?.setSelectionRange(at, at);
  }, [value]);

  // Fügt {{name}} an der Cursorposition ein; ohne Fokus hängt der Platzhalter hinten an
  const insert = (name: string) => {
    const token = `{{${name}}}`;
    const el = area.current;
    const start = el?.selectionStart ?? value.length;
    const end = el?.selectionEnd ?? value.length;
    caret.current = start + token.length;
    onChange(value.slice(0, start) + token + value.slice(end));
  };

  return (
    <section className="dispatch-prompt" role="group" aria-label={`Prompt-Vorlage ${label}`}>
      <div className="dispatch-prompt-head">
        <h3>{label}</h3>
        <Badge kind={custom ? 'unread' : 'plain'}>{custom ? 'angepasst' : 'Standard'}</Badge>
        <button type="button" className="btn" disabled={!custom} aria-label={`${label} auf Standard zurücksetzen`} onClick={() => onChange('')}>
          Auf Standard zurücksetzen
        </button>
        {defaultTemplate !== undefined && !custom && (
          <button type="button" className="btn" aria-label={`${label}: Standard übernehmen und bearbeiten`} onClick={() => onChange(defaultTemplate)}>
            Standard übernehmen und bearbeiten
          </button>
        )}
      </div>
      <textarea
        ref={area}
        className="plain-text mono"
        rows={14}
        spellCheck={false}
        aria-label={`Vorlage ${label}`}
        placeholder="leer = Standardvorlage"
        value={value}
        onChange={(e) => onChange(e.target.value)}
      />
      {placeholders.length > 0 && (
        <div className="dispatch-prompt-placeholders">
          {placeholders.map((p) => (
            <button type="button" key={p.name} className="btn small" title={p.description} onClick={() => insert(p.name)}>
              {`{{${p.name}}}`} <span className="muted">{p.description}</span>
            </button>
          ))}
        </div>
      )}
      <p className="muted">
        {required.length
          ? `Pflichtteile: ${required.join('; ')}. Ohne sie lehnt der Server die Vorlage ab.`
          : 'Vorlagen ohne die verlangten Board-Befehle lehnt der Server ab.'}
      </p>
      {problem && (problem.missing.length > 0 || problem.unknown.length > 0) && (
        <div className="banner" role="alert">
          {problem.missing.length > 0 && <div>Fehlende Pflichtteile: {problem.missing.join('; ')}</div>}
          {problem.unknown.length > 0 && <div>Unbekannte Platzhalter: {problem.unknown.map((n) => `{{${n}}}`).join(', ')}</div>}
        </div>
      )}
      <div className="dispatch-prompt-extra">{children}</div>
    </section>
  );
}
