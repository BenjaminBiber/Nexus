import { BoardSkeleton } from './skeleton/BoardSkeleton.js';
import { DetailSkeleton } from './skeleton/DetailSkeleton.js';
import { DocSkeleton } from './skeleton/DocSkeleton.js';
import { ListSkeleton } from './skeleton/ListSkeleton.js';

export type LoadingVariant = 'board' | 'list' | 'detail' | 'doc';

// Wählt den Platzhalter, der zur Form der echten Seite passt
function Skeleton({ variant }: { variant: LoadingVariant }) {
  if (variant === 'board') return <BoardSkeleton />;
  if (variant === 'detail') return <DetailSkeleton />;
  if (variant === 'doc') return <DocSkeleton />;
  return <ListSkeleton />;
}

// Ladezustand: Platzhalter in der Form der Seite plus Text für Screenreader und Beschriftung (z. B. „Tickets werden geladen …“).
export function LoadingState({ text, variant = 'list' }: { text: string; variant?: LoadingVariant }) {
  return (
    <div className="state loading" role="status" aria-busy="true">
      <Skeleton variant={variant} />
      <span className="muted sk-caption">{text}</span>
    </div>
  );
}
