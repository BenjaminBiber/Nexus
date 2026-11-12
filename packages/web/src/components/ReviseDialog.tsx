import { useEffect, useId, useState } from 'react';
import type { ReviseDocResultDto } from '../api/types.js';
import { LineDiffView } from './LineDiffView.js';
import { LoadingState } from './LoadingState.js';
import { Button } from './ui/Button.js';
import { Dialog } from './ui/Dialog.js';
import { Input } from './ui/Input.js';

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
  const titleId = useId();

  // Ein neuer Vorschlag füllt das Textfeld; danach bleibt der eigene Text erhalten
  useEffect(() => {
    if (result) setProposal(result.proposal);
  }, [result]);

  // Escape schließt den Dialog, auch während Claude noch läuft
  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') onClose();
  };

  return (
    <Dialog role="dialog" labelledBy={titleId} className="revise-dialog section-editor" onKeyDown={onKeyDown}>
      <div className="section-editor-head">
        <strong id={titleId}>Mit Claude ändern</strong>
      </div>
      <blockquote className="doc-comment-draft-quote">{quote}</blockquote>
      {running && <LoadingState text="Claude arbeitet – das kann bis zu 5 Minuten dauern" />}
      {!running && result && (
        <>
          <LineDiffView original={result.original} proposal={proposal} />
          <Input multiline aria-label="Vorschlag" value={proposal} onChange={setProposal} />
        </>
      )}
      {!running && !result && (
        <Input
          multiline
          aria-label="Änderungsanweisung"
          autoFocus
          value={instruction}
          onChange={setInstruction}
        />
      )}
      {error && <p role="alert" className="error">{error}</p>}
      <div className="doc-comment-actions">
        <Button className="small" onClick={onClose}>{result ? 'Verwerfen' : 'Abbrechen'}</Button>
        {result && !running
          ? <Button variant="primary" className="small" disabled={saving} onClick={() => onApply(proposal)}>Übernehmen</Button>
          : <Button variant="primary" className="small" disabled={running || instruction.trim() === ''} onClick={() => onRequest(instruction.trim())}>Änderung vorschlagen</Button>}
      </div>
    </Dialog>
  );
}
