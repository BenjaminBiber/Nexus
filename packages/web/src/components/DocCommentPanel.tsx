import { useState } from 'react';
import type { DocCommentDto, DocCommentPatch } from '../api/types.js';
import { DocCommentItem } from './DocCommentItem.js';

interface Props {
  comments: DocCommentDto[];
  missing: Set<string>;
  error: string | null;
  onJump: (c: DocCommentDto) => void;
  onUpdate: (id: string, patch: DocCommentPatch) => Promise<boolean>;
  onRemove: (id: string) => void;
}

// Seitenleiste mit offenen/erledigten Kommentaren und deren Aktionen
export function DocCommentPanel(props: Props) {
  const { comments, missing, error, onJump, onUpdate, onRemove } = props;
  const [tab, setTab] = useState<'open' | 'resolved'>('open');
  const open = comments.filter((c) => c.status === 'open');
  const resolved = comments.filter((c) => c.status === 'resolved');
  const shown = tab === 'open' ? open : resolved;

  return (
    <aside className="doc-comments" aria-label="Kommentare">
      {error && <p role="alert" className="error">{error}</p>}
      <div className="doc-comment-tabs">
        <button type="button" aria-pressed={tab === 'open'} onClick={() => setTab('open')}>Offen ({open.length})</button>
        <button type="button" aria-pressed={tab === 'resolved'} onClick={() => setTab('resolved')}>Erledigt ({resolved.length})</button>
      </div>
      {comments.length === 0 && <p className="muted">Markiere Text im Dokument, um ihn zu kommentieren.</p>}
      {comments.length > 0 && shown.length === 0 && (
        <p className="muted">{tab === 'open' ? 'Keine offenen Kommentare.' : 'Keine erledigten Kommentare.'}</p>
      )}
      <ul className="doc-comment-list">
        {shown.map((c) => (
          <DocCommentItem
            key={c.id}
            comment={c}
            notFound={missing.has(c.id)}
            onJump={() => onJump(c)}
            onUpdate={(patch) => onUpdate(c.id, patch)}
            onRemove={() => onRemove(c.id)}
          />
        ))}
      </ul>
    </aside>
  );
}
