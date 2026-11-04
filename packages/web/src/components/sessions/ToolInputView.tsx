import { LineDiffView } from '../LineDiffView.js';
import { inputObject, stringField } from './toolDisplay.js';

// Größere Ersetzungen nicht mehr zeilenweise vergleichen (der Vergleich wächst mit dem Produkt der Zeilenzahlen)
const DIFF_LIMIT = 250_000;

function lineCount(text: string): number {
  return text.split('\n').length;
}

// Ersetzung alt → neu als Zeilen-Diff; sehr große Ersetzungen als zwei Blöcke
function Replacement({ before, after, label }: { before: string; after: string; label: string }) {
  if (lineCount(before) * lineCount(after) > DIFF_LIMIT) {
    return (
      <div className="tx-replace">
        <pre className="tx-code" aria-label={`${label}: vorher`}>{before}</pre>
        <pre className="tx-code" aria-label={`${label}: nachher`}>{after}</pre>
      </div>
    );
  }
  return <LineDiffView original={before} proposal={after} label={label} className="tx-diff" />;
}

// Datei-Zeile über der Eingabe (Edit, Write, Read …)
function FilePath({ path }: { path: string }) {
  return <div className="tx-input-path mono">{path}</div>;
}

// Eingabe eines Tool-Aufrufs, je nach Tool aufbereitet: Bash als Befehl, Edit als Diff, Write als Inhalt, sonst JSON.
export function ToolInputView({ name, input }: { name: string; input: unknown }) {
  const obj = inputObject(input);
  const path = stringField(obj, 'file_path') ?? stringField(obj, 'notebook_path');

  if ((name === 'Bash' || name === 'PowerShell') && stringField(obj, 'command') !== null) {
    const description = stringField(obj, 'description');
    return (
      <>
        {description && <p className="tx-input-note">{description}</p>}
        <pre className="tx-code tx-command" aria-label="Befehl">{stringField(obj, 'command')}</pre>
      </>
    );
  }

  if (name === 'Edit' && stringField(obj, 'old_string') !== null && stringField(obj, 'new_string') !== null) {
    return (
      <>
        {path && <FilePath path={path} />}
        <Replacement before={stringField(obj, 'old_string')!} after={stringField(obj, 'new_string')!} label="Änderung" />
        {obj?.replace_all === true && <p className="tx-input-note">Alle Vorkommen ersetzt</p>}
      </>
    );
  }

  if (name === 'MultiEdit' && Array.isArray(obj?.edits)) {
    const edits = (obj.edits as unknown[]).map(inputObject).filter((e): e is Record<string, unknown> => e !== null);
    return (
      <>
        {path && <FilePath path={path} />}
        {edits.map((edit, index) => (
          <Replacement key={index} before={stringField(edit, 'old_string') ?? ''} after={stringField(edit, 'new_string') ?? ''} label={`Änderung ${index + 1}`} />
        ))}
      </>
    );
  }

  if (name === 'Write' && stringField(obj, 'content') !== null) {
    return (
      <>
        {path && <FilePath path={path} />}
        <pre className="tx-code" aria-label="Inhalt">{stringField(obj, 'content')}</pre>
      </>
    );
  }

  return <pre className="tx-code" aria-label="Eingabe">{pretty(input)}</pre>;
}

// JSON hübsch eingerückt; Zeichenketten so, wie sie sind
function pretty(input: unknown): string {
  if (typeof input === 'string') return input;
  try {
    return JSON.stringify(input, null, 2) ?? '';
  } catch {
    return String(input);
  }
}
