import { useEffect, useState } from 'react';
import type { ReviseDocResultDto } from '../api/types.js';
import { LineDiffView } from './LineDiffView.js';
import { LoadingState } from './LoadingState.js';

interface Props {
  quote: string;
  running: boolean;
  result: ReviseDocResultDto | null;
  error: string | null;
  saving: boolean;
  onRequest: (instruction: string) => void;
  onApply: (text: string) => void;
  onClose: () => void;
}

// Dialog „Mit Claude ändern“: Anweisung eingeben, warten, Vorschlag als Diff prüfen, bearbeiten und übernehmen
export function ReviseDialog({ quote, running, result, error, saving, onRequest, onApply, onClose }: Props) {
  const [instruction, setInstruction] = useState('');
  const [proposal, setProposal] = useState('');

  // Ein neuer Vorschlag füllt das Textfeld; danach bleibt der eigene Text erhalten
  useEffect(() => {
    if (result) setProposal(result.proposal);
  }, [result]);

  // Escape schließt den Dialog, auch während Claude noch läuft
  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') onClose();
  };

  return (
    <div className="revise-dialog section-editor" role="dialog" aria-label="Mit Claude ändern" onKeyDown={onKeyDown}>
      <div className="section-editor-head">
        <strong>Mit Claude ändern</strong>
      </div>
      <blockquote className="doc-comment-draft-quote">{quote}</blockquote>
      {running && <LoadingState variant="detail" text="Claude arbeitet – das kann bis zu 5 Minuten dauern" />}
      {!running && result && (
        <>
          <LineDiffView original={result.original} proposal={proposal} />
          <textarea aria-label="Vorschlag" value={proposal} onChange={(e) => setProposal(e.target.value)} />
        </>
      )}
      {!running && !result && (
        <textarea
          aria-label="Änderungsanweisung"
          autoFocus
          value={instruction}
          onChange={(e) => setInstruction(e.target.value)}
        />
      )}
      {error && <p role="alert" className="error">{error}</p>}
      <div className="doc-comment-actions">
        <button type="button" className="btn small" onClick={onClose}>{result ? 'Verwerfen' : 'Abbrechen'}</button>
        {result && !running
          ? <button type="button" className="btn small primary" disabled={saving} onClick={() => onApply(proposal)}>Übernehmen</button>
          : <button type="button" className="btn small primary" disabled={running || instruction.trim() === ''} onClick={() => onRequest(instruction.trim())}>Änderung vorschlagen</button>}
      </div>
    </div>
  );
}
