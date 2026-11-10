import { forwardRef, useEffect, useId, useImperativeHandle, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import { useBoards } from '@nexus/theme-sdk';
import { boardKey, commonPrefix, completions, runCommand, slug, type CompletionData, type OutputBlock, type OutputCell, type TerminalActions } from './terminalCommands.js';

/** Steuerung der Prompt-Zeile von außen (Taste „/“ im Rahmen) */
export interface TerminalPromptHandle {
  focus(): void;
}

interface Output {
  command: string;
  blocks: OutputBlock[];
}

const HISTORY_LIMIT = 100;

// „nexus@board“ mit dem gewählten Board (sonst dem Projekt); den Namen des Nutzers kennt das Theme-SDK nicht
function useIdentity(): string {
  const { current, boards, projects } = useBoards();
  const board = current?.board == null ? undefined : boards.find((b) => b.id === current.board);
  const project = current ? projects.find((p) => p.id === current.project) : undefined;
  return `nexus@${slug(board?.name ?? project?.name ?? '') || 'nexus'}`;
}

// Boards für die Ergänzung, aus der Board-Wahl des Hosts
function useCompletionData(actions: TerminalActions): CompletionData {
  const { boards } = useBoards();
  return useMemo(() => {
    const all = actions.boards();
    return { boards: all.map((b) => boardKey(b, all)) };
    // actions liest den neuesten Stand; neu rechnen, sobald sich die Boards des Hosts ändern
  }, [actions, boards]);
}

function Cell({ cell }: { cell: OutputCell }) {
  return <span className={`term-tone-${cell.tone ?? 'plain'}`}>{cell.text}</span>;
}

function Block({ block }: { block: OutputBlock }) {
  if (block.kind === 'line') {
    return (
      <div className="term-out-line">
        {block.cells.map((cell, i) => <Cell key={i} cell={cell} />)}
      </div>
    );
  }
  return (
    <table className="term-out-table">
      <tbody>
        {block.rows.map((row, r) => (
          <tr key={r}>
            {row.map((cell, c) => <td key={c}><Cell cell={cell} /></td>)}
          </tr>
        ))}
      </tbody>
    </table>
  );
}

// Prompt-Zeile des Schemas „Terminal“: „nexus@board:~/pfad$“ und eine Eingabe für Befehle (help zeigt sie).
// Tab oder → übernimmt den grauen Vorschlag, ↑/↓ blättert im Verlauf, Esc und Strg+L leeren Zeile und Ausgabe.
export const TerminalPrompt = forwardRef<TerminalPromptHandle, { actions: TerminalActions; path: string }>(function TerminalPrompt({ actions, path }, ref) {
  const id = useId();
  const input = useRef<HTMLInputElement>(null);
  const area = useRef<HTMLDivElement>(null);
  const identity = useIdentity();
  const completionData = useCompletionData(actions);
  const [value, setValue] = useState('');
  const [caretAtEnd, setCaretAtEnd] = useState(true);
  const [output, setOutput] = useState<Output | null>(null);
  const [busy, setBusy] = useState(false);
  const history = useRef<string[]>([]);
  const historyIndex = useRef(-1);

  useImperativeHandle(ref, () => ({ focus: () => input.current?.focus() }), []);

  // Klick außerhalb der Zeile schließt die Ausgabe
  useEffect(() => {
    if (!output) return;
    const onDown = (event: PointerEvent) => {
      if (!area.current?.contains(event.target as Node)) setOutput(null);
    };
    document.addEventListener('pointerdown', onDown);
    return () => document.removeEventListener('pointerdown', onDown);
  }, [output]);

  const options = value ? completions(value, completionData) : [];
  const suggestion = options.find((o) => o.toLowerCase().startsWith(value.toLowerCase()) && o.length > value.length);
  const rest = suggestion && caretAtEnd ? suggestion.slice(value.length) : '';

  const change = (next: string) => {
    setValue(next);
    setCaretAtEnd(true);
  };

  const trackCaret = () => {
    const el = input.current;
    if (el) setCaretAtEnd(el.selectionStart === el.value.length && el.selectionEnd === el.value.length);
  };

  const submit = async () => {
    const command = value.trim();
    change('');
    historyIndex.current = -1;
    if (!command) return;
    history.current = [...history.current.filter((h) => h !== command), command].slice(-HISTORY_LIMIT);
    setBusy(true);
    try {
      const blocks = await runCommand(command, actions);
      setOutput(blocks ? { command, blocks } : null);
    } finally {
      setBusy(false);
    }
  };

  const complete = () => {
    if (!options.length) return;
    if (options.length === 1) return change(`${options[0]} `);
    const common = commonPrefix(options);
    if (common.length > value.length) return change(common);
    setOutput({ command: `${value}⇥`, blocks: [{ kind: 'line', cells: options.map((o) => ({ text: `${o.split(/\s+/).pop()}  ` })) }] });
  };

  const browseHistory = (step: -1 | 1) => {
    const entries = history.current;
    if (!entries.length) return;
    const start = historyIndex.current === -1 ? entries.length : historyIndex.current;
    const next = start + step;
    historyIndex.current = next >= entries.length ? -1 : Math.max(0, next);
    change(historyIndex.current === -1 ? '' : entries[historyIndex.current]!);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Enter') {
      event.preventDefault();
      if (!busy) void submit();
    } else if (event.key === 'Tab' && value) {
      event.preventDefault();
      complete();
    } else if (event.key === 'ArrowRight' && rest) {
      event.preventDefault();
      change(value + rest);
    } else if (event.key === 'ArrowUp' || event.key === 'ArrowDown') {
      event.preventDefault();
      browseHistory(event.key === 'ArrowUp' ? -1 : 1);
    } else if (event.key === 'Escape' || (event.key.toLowerCase() === 'l' && event.ctrlKey)) {
      event.preventDefault();
      change('');
      setOutput(null);
    }
  };

  return (
    <div ref={area} className="term-prompt">
      <label className="term-prompt-label" htmlFor={id}>
        <span className="term-user">{identity}</span>:<span className="term-path">~/{path}</span>
        <span className="term-sigil">$</span>
      </label>
      <span className="term-cmd">
        <input
          ref={input}
          id={id}
          className={`term-input${caretAtEnd ? ' caret-at-end' : ''}`}
          value={value}
          aria-label="Befehl (help zeigt alle Befehle)"
          aria-busy={busy}
          autoComplete="off"
          spellCheck={false}
          onChange={(e) => {
            setValue(e.target.value);
            historyIndex.current = -1;
            trackCaret();
          }}
          onKeyDown={onKeyDown}
          onKeyUp={trackCaret}
          onClick={trackCaret}
          onSelect={trackCaret}
        />
        <span className="term-ghost" aria-hidden="true">
          <span className="term-typed">{value}</span>
          <span className="term-cursor" />
          <span className="term-suggestion">{rest}</span>
        </span>
      </span>
      {output && (
        <div className="term-output" role="log" aria-live="polite" aria-label="Ausgabe">
          <div className="term-out-line term-echo">
            <span className="term-tone-muted">$ </span>
            <span>{output.command}</span>
          </div>
          {output.blocks.map((block, i) => <Block key={i} block={block} />)}
          <div className="term-out-line term-tone-hint">Esc schließt</div>
        </div>
      )}
    </div>
  );
});
