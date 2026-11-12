import { memo, useId, useState } from 'react';
import type { TranscriptEntry } from '../../api/sessionDetailTypes.js';
import { MarkdownView } from '../../markdown/MarkdownView.js';
import { formatClock, formatDateTime, formatNumber, formatSeconds } from '../../sessions/format.js';
import { ToolInputView } from './ToolInputView.js';
import { toolDetail, toolDisplayName, toolIcon } from './toolDisplay.js';

type Entry<K extends TranscriptEntry['kind']> = Extract<TranscriptEntry, { kind: K }>;

// Ab dieser Länge wird ein Prompt zunächst gekürzt gezeigt
const PROMPT_CLAMP_LINES = 12;
const PROMPT_CLAMP_CHARS = 1200;

function Icon({ name, className = '' }: { name: string; className?: string }) {
  return (
    <span className={`material-icons ${className}`.trim()} aria-hidden="true">
      {name}
    </span>
  );
}

// Prompt des Nutzers als abgesetzte Blase rechts; lange Prompts aufklappbar
function PromptEntry({ entry }: { entry: Entry<'prompt'> }) {
  const long = entry.text.split('\n').length > PROMPT_CLAMP_LINES || entry.text.length > PROMPT_CLAMP_CHARS;
  const [open, setOpen] = useState(false);
  const bodyId = useId();
  return (
    <div className="tx-prompt">
      <div className="tx-prompt-meta">
        <span className="tx-who">Du</span>
        <time dateTime={entry.at} title={formatDateTime(entry.at)}>
          {formatClock(entry.at)}
        </time>
      </div>
      <div id={bodyId} className={`tx-bubble${long && !open ? ' clamped' : ''}`}>
        {entry.text}
      </div>
      {long && (
        <button type="button" className="tx-more" aria-expanded={open} aria-controls={bodyId} onClick={() => setOpen((o) => !o)}>
          {open ? 'Weniger anzeigen' : 'Ganzen Prompt anzeigen'}
        </button>
      )}
    </div>
  );
}

// Antwort als Markdown
function TextEntry({ entry }: { entry: Entry<'text'> }) {
  return (
    <div className="tx-text" title={formatDateTime(entry.at)}>
      <MarkdownView source={entry.text} />
    </div>
  );
}

// Thinking eingeklappt: „Nachgedacht · 2 s“; ohne Text nur als Hinweis
function ThinkingEntry({ entry }: { entry: Entry<'thinking'> }) {
  const [open, setOpen] = useState(false);
  const bodyId = useId();
  const label = entry.durationMs !== null ? `Nachgedacht · ${formatSeconds(entry.durationMs)}` : 'Nachgedacht';
  const hasText = entry.text.trim() !== '';
  if (!hasText) {
    return (
      <div className="tx-thinking">
        <span className="tx-thinking-head tx-static">
          <Icon name="psychology" />
          {label}
        </span>
      </div>
    );
  }
  return (
    <div className="tx-thinking">
      <button type="button" className="tx-thinking-head" aria-expanded={open} aria-controls={bodyId} onClick={() => setOpen((o) => !o)}>
        <Icon name="psychology" />
        {label}
        <Icon name="expand_more" className="tx-chevron" />
      </button>
      {open && (
        <div id={bodyId} className="tx-thinking-body">
          <MarkdownView source={entry.text} />
        </div>
      )}
    </div>
  );
}

// Tool-Aufruf als kompakte Zeile (Symbol, Name, Kurzinfo, Zustand); aufgeklappt Eingabe und Ergebnis
function ToolEntry({ entry }: { entry: Entry<'tool'> }) {
  const [open, setOpen] = useState(false);
  const bodyId = useId();
  const name = toolDisplayName(entry.name);
  const detail = toolDetail(entry.name, entry.summary);
  const failed = entry.result?.isError === true;
  const pending = entry.result === null;
  return (
    <div className={`tx-tool${failed ? ' failed' : ''}${open ? ' open' : ''}`}>
      <button type="button" className="tx-tool-head" aria-expanded={open} aria-controls={bodyId} onClick={() => setOpen((o) => !o)} title={formatDateTime(entry.at)}>
        <Icon name={failed ? 'error' : toolIcon(entry.name)} className="tx-tool-icon" />
        <span className="tx-tool-name">{name}</span>
        {detail && <span className="tx-tool-summary">{detail}</span>}
        {failed && <span className="tx-tool-state error">Fehler</span>}
        {pending && <span className="tx-tool-state pending">ohne Ergebnis</span>}
        <Icon name="expand_more" className="tx-chevron" />
      </button>
      {open && (
        <div id={bodyId} className="tx-tool-body">
          <section aria-label="Eingabe">
            <h4>Eingabe</h4>
            <ToolInputView name={entry.name} input={entry.input} />
          </section>
          <section aria-label="Ergebnis">
            <h4>Ergebnis</h4>
            {entry.result === null ? (
              <p className="muted">Noch kein Ergebnis.</p>
            ) : (
              <>
                <pre className={`tx-code tx-result${failed ? ' error' : ''}`}>{entry.result.text.trim() === '' ? '(keine Ausgabe)' : entry.result.text}</pre>
                {entry.result.truncated && <p className="tx-input-note">Ausgabe gekürzt (die ersten {formatNumber(20000)} Zeichen).</p>}
              </>
            )}
          </section>
        </div>
      )}
    </div>
  );
}

const SYSTEM_ICON = { info: 'info', warning: 'warning', error: 'error' } as const;

// Systemhinweis (Hook-Fehler, Kompaktierung …) dezent
function SystemEntry({ entry }: { entry: Entry<'system'> }) {
  return (
    <div className={`tx-system ${entry.level}`} title={formatDateTime(entry.at)}>
      <Icon name={SYSTEM_ICON[entry.level]} />
      <span>{entry.text}</span>
    </div>
  );
}

// Ein Eintrag des Verlaufs. Memo: beim Nachladen bleiben bekannte Einträge dieselben Objekte und rendern nicht neu.
export const TranscriptEntryView = memo(function TranscriptEntryView({ entry }: { entry: TranscriptEntry }) {
  return (
    <li className={`tx-entry ${entry.kind}`} data-kind={entry.kind}>
      {entry.kind === 'prompt' && <PromptEntry entry={entry} />}
      {entry.kind === 'text' && <TextEntry entry={entry} />}
      {entry.kind === 'thinking' && <ThinkingEntry entry={entry} />}
      {entry.kind === 'tool' && <ToolEntry entry={entry} />}
      {entry.kind === 'system' && <SystemEntry entry={entry} />}
    </li>
  );
});
